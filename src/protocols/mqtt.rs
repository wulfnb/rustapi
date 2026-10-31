use super::*;
use anyhow::Context;
use rumqttc::{AsyncClient, Event as Notification, MqttOptions, Packet, QoS, Transport};
pub(super) async fn run(
    profile: &Session,
    commands: Option<&Receiver<String>>,
    sink: &mut Sink<'_>,
) -> Result<()> {
    let url = reqwest::Url::parse(&profile.request.url)?;
    ensure!(
        ["mqtt", "mqtts"].contains(&url.scheme()),
        "MQTT URL must use mqtt:// or mqtts://"
    );
    ensure!(profile.qos <= 2, "MQTT QoS must be 0, 1 or 2");
    ensure!(
        profile.request.settings.verify_tls
            && profile.request.settings.proxy.is_empty()
            && profile.request.settings.ca_file.is_empty()
            && profile.request.settings.identity_file.is_empty(),
        "MQTT currently requires normal verified TLS without custom CA, identity or proxy settings"
    );
    let host = url.host_str().context("MQTT hostname missing")?;
    let id = if profile.mqtt_client_id.is_empty() {
        format!("rustapi-{}", uuid::Uuid::new_v4().simple())
    } else {
        profile.mqtt_client_id.clone()
    };
    let mut options = MqttOptions::new(
        id,
        host,
        url.port()
            .unwrap_or(if url.scheme() == "mqtts" { 8883 } else { 1883 }),
    );
    options
        .set_keep_alive(Duration::from_secs(30))
        .set_max_packet_size(1024 * 1024, 1024 * 1024);
    if !profile.mqtt_username.is_empty() {
        options.set_credentials(&profile.mqtt_username, &profile.mqtt_password);
    }
    if url.scheme() == "mqtts" {
        options.set_transport(Transport::tls_with_config(
            rumqttc::TlsConfiguration::Native,
        ));
    }
    let qos = match profile.qos {
        0 => QoS::AtMostOnce,
        1 => QoS::AtLeastOnce,
        _ => QoS::ExactlyOnce,
    };
    let (client, mut eventloop) = AsyncClient::new(options, 16);
    if profile.subscribe {
        client.subscribe(&profile.topic, qos).await?;
    }
    let producer = client.clone();
    let topic = profile.topic.clone();
    let retain = profile.retain;
    let messages = profile.messages.clone();
    let mut sender = tokio::spawn(async move {
        for message in messages {
            producer
                .publish(&topic, qos, retain, message.into_bytes())
                .await?;
        }
        Ok::<_, rumqttc::ClientError>(())
    });
    let mut poll = tokio::time::interval(Duration::from_millis(20));
    let mut sender_done = false;
    loop {
        // poll() can be in the middle of TCP/TLS/MQTT connection setup. Keep
        // that future alive when a command timer tick wins the select.
        let notification = {
            let notification = eventloop.poll();
            tokio::pin!(notification);
            loop {
                tokio::select! {
                    result = &mut notification => break result.context("MQTT broker event")?,
                    _ = poll.tick() => {
                        if !sender_done && sender.is_finished() {
                            (&mut sender).await??;
                            sender_done = true;
                        }
                        if let Some(commands) = commands {
                            if let Ok(message) = commands.try_recv() {
                                ensure!(message.len() <= 1024 * 1024, "MQTT message exceeds 1 MiB");
                                client.try_publish(&profile.topic, qos, profile.retain, message.into_bytes())?;
                            }
                        }
                    }
                }
            }
        };
        let complete = match notification {
            Notification::Incoming(Packet::ConnAck(_)) => {
                sink.send("connected", json!({"transport":"mqtt","qos":profile.qos}))?
            }
            Notification::Incoming(Packet::Publish(packet)) => {
                use base64::Engine;
                sink.send("message", json!({"topic":packet.topic,"qos":packet.qos as u8,"retain":packet.retain,"base64":base64::engine::general_purpose::STANDARD.encode(packet.payload)}))?
            }
            Notification::Incoming(Packet::PubAck(_) | Packet::PubComp(_))
                if !profile.subscribe =>
            {
                sink.send(
                    "published",
                    json!({"topic":profile.topic,"qos":profile.qos}),
                )?
            }
            Notification::Outgoing(rumqttc::Outgoing::Publish(_))
                if profile.qos == 0 && !profile.subscribe =>
            {
                sink.send("published", json!({"topic":profile.topic,"qos":0}))?
            }
            _ => false,
        };
        if complete {
            sender.abort();
            return Ok(());
        }
    }
}
