use egui::{accesskit, pos2, vec2, Context, Event, Modifiers, PointerButton, RawInput, Rect};
use std::sync::{Mutex, MutexGuard};

static PROFILE_LOCK: Mutex<()> = Mutex::new(());

pub struct Profile {
    pub directory: tempfile::TempDir,
    _lock: MutexGuard<'static, ()>,
}

impl Profile {
    pub fn new() -> Self {
        let lock = PROFILE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let directory = tempfile::tempdir().unwrap();
        std::env::set_var("RUSTAPI_DATA_DIR", directory.path());
        rustapi::storage::init_storage().unwrap();
        Self {
            directory,
            _lock: lock,
        }
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        std::env::remove_var("RUSTAPI_DATA_DIR");
    }
}

pub struct Harness<S, F> {
    pub state: S,
    pub ctx: Context,
    pub output: egui::FullOutput,
    draw: F,
    frame: u64,
}

impl<S, F: FnMut(&Context, &mut S)> Harness<S, F> {
    pub fn new(state: S, draw: F) -> Self {
        let ctx = Context::default();
        ctx.enable_accesskit();
        ctx.style_mut(|s| s.animation_time = 0.0);
        let mut harness = Self {
            state,
            ctx,
            output: Default::default(),
            draw,
            frame: 0,
        };
        harness.refresh();
        harness
    }

    pub fn input(&mut self, events: Vec<Event>) {
        self.frame += 1;
        self.output = self.ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1800.0, 1600.0))),
                time: Some(self.frame as f64 / 60.0),
                events,
                ..Default::default()
            },
            |ctx| (self.draw)(ctx, &mut self.state),
        );
    }

    pub fn refresh(&mut self) {
        for _ in 0..3 {
            self.input(vec![]);
        }
    }

    pub fn nodes(&self) -> &[(accesskit::NodeId, accesskit::Node)] {
        &self
            .output
            .platform_output
            .accesskit_update
            .as_ref()
            .unwrap()
            .nodes
    }

    pub fn rect_n(&self, name: &str, index: usize) -> Rect {
        let mut candidates: Vec<_> = self
            .nodes()
            .iter()
            .filter(|(_, node)| {
                (node.label() == Some(name) || node.value() == Some(name))
                    && node.bounds().is_some()
            })
            .collect();
        candidates.sort_by(|(_, a), (_, b)| {
            let a = a.bounds().unwrap();
            let b = b.bounds().unwrap();
            a.y0.total_cmp(&b.y0).then(a.x0.total_cmp(&b.x0))
        });
        let (_, node) = candidates.get(index).unwrap_or_else(|| {
            panic!(
                "Missing widget {name:?} #{index}. Nodes: {:?}",
                self.nodes()
                    .iter()
                    .map(|(_, n)| (n.role(), n.label().or(n.value()), n.value()))
                    .collect::<Vec<_>>()
            )
        });
        let bounds = node.bounds().unwrap();
        Rect::from_min_max(
            pos2(bounds.x0 as f32, bounds.y0 as f32),
            pos2(bounds.x1 as f32, bounds.y1 as f32),
        )
    }

    pub fn click(&mut self, name: &str) {
        self.click_n(name, 0);
    }

    pub fn click_n(&mut self, name: &str, index: usize) {
        self.refresh();
        let rect = self.rect_n(name, index);
        self.click_rect(rect, PointerButton::Primary);
    }

    pub fn right_click(&mut self, name: &str) {
        self.right_click_n(name, 0);
    }

    pub fn right_click_n(&mut self, name: &str, index: usize) {
        self.refresh();
        let rect = self.rect_n(name, index);
        self.click_rect(rect, PointerButton::Secondary);
        self.refresh();
    }

    pub fn click_rect(&mut self, rect: Rect, button: PointerButton) {
        let pos = rect.center();
        self.input(vec![
            Event::PointerMoved(pos),
            Event::PointerButton {
                pos,
                button,
                pressed: true,
                modifiers: Modifiers::default(),
            },
        ]);
        self.input(vec![Event::PointerButton {
            pos,
            button,
            pressed: false,
            modifiers: Modifiers::default(),
        }]);
    }

    pub fn key(&mut self, key: egui::Key) {
        self.input(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::default(),
        }]);
        self.input(vec![Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers: Modifiers::default(),
        }]);
    }

    pub fn combo(&mut self, index: usize) {
        self.refresh();
        self.click_rect(
            self.role_rect(accesskit::Role::ComboBox, index),
            PointerButton::Primary,
        );
        self.refresh();
    }

    pub fn role_rect(&self, role: accesskit::Role, index: usize) -> Rect {
        let mut bounds: Vec<_> = self
            .nodes()
            .iter()
            .filter(|(_, n)| n.role() == role)
            .filter_map(|(_, n)| n.bounds())
            .collect();
        bounds.sort_by(|a, b| a.y0.total_cmp(&b.y0).then(a.x0.total_cmp(&b.x0)));
        let b = bounds
            .get(index)
            .unwrap_or_else(|| panic!("Missing role {role:?} #{index}"));
        Rect::from_min_max(
            pos2(b.x0 as f32, b.y0 as f32),
            pos2(b.x1 as f32, b.y1 as f32),
        )
    }

    pub fn edit(&mut self, role: accesskit::Role, index: usize, text: &str) {
        self.click_rect(self.role_rect(role, index), PointerButton::Primary);
        self.input(vec![
            Event::Key {
                key: egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
            },
            Event::Text(text.into()),
        ]);
    }
}
