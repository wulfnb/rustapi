"""Compile/run additional SDK exports against loopback fixtures, including policies.

Requires Go, .NET 8 SDK, Java 17 and Dart 3. RUSTAPI_GO/DOTNET/JAVA/JAVAC/DART
can name SDK binaries. Build examples first. Optional RUSTAPI_CODEGEN_CACHE keeps
SDK caches between runs; generated source/projects are always temporary.
"""
import http.server
import json
import os
from pathlib import Path
import shutil
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
import time
from urllib.parse import parse_qs, urlsplit

ROOT = Path(__file__).resolve().parents[1]
BUILD = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug").resolve()
RECORDS = []
SERVERS = []
CROSS_TARGET = ""

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        uri = urlsplit(self.path)
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        RECORDS.append((uri.path[1:], parse_qs(uri.query), self.headers, body))
        if uri.path.startswith(("/redirect/", "/cross/")):
            kind, target = uri.path[1:].split("/", 1)
            self.send_response(307)
            self.send_header("Location", (CROSS_TARGET if kind == "cross" else "") + "/" + target + "?" + uri.query)
            self.send_header("Content-Length", "0")
            self.send_header("Connection", "close")
            self.end_headers()
            return
        self.send_response(200)
        self.send_header("Content-Length", "11")
        self.send_header("Connection", "close")
        self.end_headers()
        if uri.path.startswith("/slow/"):
            time.sleep(3)
        try:
            self.wfile.write(b'{"ok":true}')
        except (BrokenPipeError, ConnectionResetError):
            pass
    do_OPTIONS = do_POST
    def log_message(self, *_args):
        pass

def server(context=None):
    instance = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    if context:
        instance.socket = context.wrap_socket(instance.socket, server_side=True)
    SERVERS.append(instance)
    threading.Thread(target=instance.serve_forever, daemon=True).start()
    return instance, f"{'https' if context else 'http'}://127.0.0.1:{instance.server_address[1]}"

def run(command, env, failure=False, cwd=None):
    result = subprocess.run(command, capture_output=True, text=True, timeout=180, env=env, cwd=cwd)
    if (result.returncode != 0) != failure:
        raise RuntimeError(f"{command[0]} returned {result.returncode}:\n{result.stdout}\n{result.stderr}")
    return result

def validate(case, record, api_key=True):
    actual, query, headers, body = record
    expected = {"q": ["a b&c"]}
    if api_key:
        expected["api_key"] = ["key with spaces"]
    assert actual == case and query == expected, (case, actual, query)
    assert headers["x-quote"] == 'quote-"-dollar-$(echo nope)', (case, headers)
    if case == "json":
        assert json.loads(body)["ready"] is True and json.loads(body)["nil"] is None
    elif case == "text":
        assert b"${never}" in body and b"\x01" in body and "🦀" in body.decode()
        assert "\u0085\u2028\u2029" in body.decode()
    elif case == "form":
        assert b"Ada & friends" in body and bytes([0, 255, 128, 1, 13, 10]) in body and bytes([7, 6, 5, 4]) in body
        assert body.count(b'name="upload"') == 2
        assert body.lower().count(b"content-type: application/x-fixture") == 2
        assert b"content-type: text/plain; charset=utf-8" in body.lower()
        assert "multipart/form-data; boundary=" in headers["content-type"]
    elif case == "urlencoded":
        assert parse_qs(body.decode()) == {"name": ["Ada & friends"]}
        assert headers["content-type"].startswith("application/x-www-form-urlencoded")
    elif case == "binary":
        assert body == bytes([0, 255, 128, 1, 13, 10])
    elif case == "headers":
        assert headers.get_all("x-repeat") == ["one", "two"], headers.get_all("x-repeat")

def main():
    global CROSS_TARGET
    primary, base = server()
    _, CROSS_TARGET = server()
    with tempfile.TemporaryDirectory(prefix="rustapi-codegen-targets-") as temporary:
        directory = Path(temporary)
        env = dict(os.environ)
        cache = Path(env.get("RUSTAPI_CODEGEN_CACHE", str(directory / "cache")))
        cache.mkdir(parents=True, exist_ok=True)
        env.update({"GOCACHE": str(cache / "go-cache"), "GOPATH": str(cache / "go-home"),
                    "TEST_TELEMETRY_DIR": str(cache / "go-telemetry"), "DOTNET_CLI_HOME": str(cache / "dotnet-home"),
                    "DOTNET_CLI_TELEMETRY_OPTOUT": "1", "DOTNET_GENERATE_ASPNET_CERTIFICATE": "false", "DOTNET_NOLOGO": "1",
                    "NUGET_PACKAGES": str(cache / "nuget"), "DART_SUPPRESS_ANALYTICS": "true"})
        (cache / "go-telemetry").mkdir(exist_ok=True)
        (cache / "go-telemetry/mode").write_text("off\n")
        tools = {name: env.get(f"RUSTAPI_{name.upper()}", name) for name in ("go", "dotnet", "java", "javac", "dart")}
        project = directory / "csharp"
        project.mkdir()
        (project / "request.csproj").write_text('<Project Sdk="Microsoft.NET.Sdk"><PropertyGroup><OutputType>Exe</OutputType><TargetFramework>net8.0</TargetFramework></PropertyGroup></Project>')
        (project / "NuGet.Config").write_text('<configuration><packageSources><clear /></packageSources></configuration>')
        (project / "global.json").write_text(json.dumps({"sdk": {"version": "8.0.100", "rollForward": "latestFeature"}}))

        def fixture(name, target, profile=None):
            output = directory / name
            command = [str(BUILD / "examples/codegen-fixture"), str(output), target]
            if profile is not None:
                path = directory / f"{name}.json"
                path.write_text(json.dumps(profile))
                command.append(str(path))
            run(command, env)
            return output

        def execute(source, language, failure=False):
            if language == "go":
                return run([tools["go"], "run", str(source)], env, failure)
            if language == "cs":
                shutil.copyfile(source, project / "Program.cs")
                run([tools["dotnet"], "build", str(project / "request.csproj"), "--nologo", "--configfile", str(project / "NuGet.Config")], env, cwd=project)
                return run([tools["dotnet"], str(project / "bin/Debug/net8.0/request.dll")], env, failure)
            if language == "java":
                target = directory / "RequestSample.java"
                shutil.copyfile(source, target)
                run([tools["javac"], "-encoding", "UTF-8", "-d", str(directory), str(target)], env)
                return run([tools["java"], "-cp", str(directory), "RequestSample"], env, failure)
            if language == "dart":
                return run([tools["dart"], str(source)], env, failure)
            with socket.create_connection(primary.server_address, timeout=5) as connection:
                connection.sendall(source.read_bytes())
                while connection.recv(8192):
                    pass
            return None

        ordinary = fixture("ordinary", base)
        for case in ("json", "text", "form", "urlencoded", "binary", "options", "headers"):
            for language in ("go", "cs", "java", "dart", "http"):
                source = ordinary / f"request-{case}.{language}"
                if language == "cs" and case == "headers":
                    assert source.read_text().startswith("Cannot generate code:")
                    continue
                if language == "http" and case in ("form", "binary"):
                    assert "cannot inline upload files" in source.read_text()
                    continue
                before = len(RECORDS)
                execute(source, language)
                assert len(RECORDS) == before + 1, (case, language, "request count")
                validate(case, RECORDS[-1])
                print(f"PASS {language} {case}", flush=True)

        ca = directory / "ca.pem"
        ca_key = directory / "ca-key.pem"
        cert = directory / "server.pem"
        key = directory / "server-key.pem"
        ca_config = directory / "ca.cnf"
        ca_config.write_text("[req]\ndistinguished_name=dn\nx509_extensions=ext\nprompt=no\n[dn]\nCN=Temporary SDK test CA\n[ext]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n")
        run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-config", str(ca_config), "-keyout", str(ca_key), "-out", str(ca)], env)
        csr = directory / "server.csr"
        run(["openssl", "req", "-newkey", "rsa:2048", "-nodes", "-subj", "/CN=localhost", "-keyout", str(key), "-out", str(csr)], env)
        extensions = directory / "server.ext"
        extensions.write_text("subjectAltName=DNS:localhost,IP:127.0.0.1\nbasicConstraints=critical,CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\nextendedKeyUsage=serverAuth\n")
        run(["openssl", "x509", "-req", "-in", str(csr), "-CA", str(ca), "-CAkey", str(ca_key), "-CAcreateserial", "-days", "1", "-extfile", str(extensions), "-out", str(cert)], env)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(cert, key)
        _, tls_base = server(context)
        def certificate(name, usage, hostname):
            configuration = directory / f"{name}.cnf"
            configuration.write_text(f"[req]\ndistinguished_name=dn\nx509_extensions=ext\nprompt=no\n[dn]\nCN={hostname}\n[ext]\nsubjectAltName=DNS:{hostname}\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,digitalSignature,keyEncipherment,keyCertSign\nextendedKeyUsage={usage}\n")
            certificate_path, key_path = directory / f"{name}.pem", directory / f"{name}-key.pem"
            run(["openssl", "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-config", str(configuration), "-keyout", str(key_path), "-out", str(certificate_path)], env)
            return certificate_path, key_path
        wrong_cert, wrong_key = certificate("wrong-host", "serverAuth", "wrong.invalid")
        wrong_csr = directory / "wrong-host.csr"
        run(["openssl", "req", "-new", "-key", str(wrong_key), "-subj", "/CN=wrong.invalid", "-out", str(wrong_csr)], env)
        wrong_extensions = directory / "wrong-host.ext"
        wrong_extensions.write_text(extensions.read_text().replace("DNS:localhost,IP:127.0.0.1", "DNS:wrong.invalid"))
        run(["openssl", "x509", "-req", "-in", str(wrong_csr), "-CA", str(ca), "-CAkey", str(ca_key), "-CAcreateserial", "-days", "1", "-extfile", str(wrong_extensions), "-out", str(wrong_cert)], env)
        wrong_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        wrong_context.load_cert_chain(wrong_cert, wrong_key)
        _, wrong_base = server(wrong_context)
        wrong = fixture("wrong", wrong_base, {"settings": {"ca_file": str(ca)}})
        client_cert, client_key = certificate("client", "clientAuth", "fixture-client")
        identity = directory / "identity.pem"
        identity.write_bytes(client_cert.read_bytes() + client_key.read_bytes())
        mutual_context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        mutual_context.load_cert_chain(cert, key)
        mutual_context.load_verify_locations(client_cert)
        mutual_context.verify_mode = ssl.CERT_REQUIRED
        _, mutual_base = server(mutual_context)
        mutual = fixture("mutual", mutual_base, {"settings": {"ca_file": str(ca), "identity_file": str(identity)}})
        trusted = fixture("trusted", tls_base, {"settings": {"ca_file": str(ca)}})
        insecure = fixture("insecure", tls_base, {"settings": {"verify_tls": False}})
        proxy = fixture("proxy", "http://fixture.invalid", {"settings": {"proxy": base}})
        limited = fixture("limited", base, {"settings": {"max_response_bytes": 4}})
        slow = fixture("slow", base + "/slow", {"settings": {"timeout_ms": 1200}})
        bearer = {"auth_type": "BearerToken", "bearer_token": "redirect-private", "basic_username": "", "basic_password": "", "api_key_name": "", "api_key_value": "", "api_key_location": "Header"}
        redirect_profile = {"auth": bearer, "headers": [{"key": "Cookie", "value": "session=redirect-private", "enabled": True, "description": ""}]}
        redirect = fixture("redirect", base + "/redirect", redirect_profile)
        cross = fixture("cross", base + "/cross", redirect_profile)
        for language in ("go", "cs", "java", "dart"):
            before = len(RECORDS)
            execute(wrong / f"request-json.{language}", language, failure=True)
            assert len(RECORDS) == before, (language, "wrong hostname accepted")
            print(f"PASS {language} rejects wrong hostname", flush=True)
            if language == "java":
                assert "direct PEM identity export is unsupported" in (mutual / "request-json.java").read_text()
            else:
                before = len(RECORDS)
                execute(mutual / f"request-json.{language}", language)
                assert len(RECORDS) == before + 1
                validate("json", RECORDS[-1])
                print(f"PASS {language} mutual TLS", flush=True)
            for name, output in (("trusted", trusted), ("proxy", proxy)):
                before = len(RECORDS)
                execute(output / f"request-json.{language}", language)
                assert len(RECORDS) == before + 1
                validate("json", RECORDS[-1])
                print(f"PASS {language} {name}", flush=True)
            if language == "java":
                assert "keeps TLS verification" in (insecure / "request-json.java").read_text()
            else:
                before = len(RECORDS)
                execute(insecure / f"request-json.{language}", language)
                assert len(RECORDS) == before + 1
                validate("json", RECORDS[-1])
                print(f"PASS {language} insecure", flush=True)
            for name, output in (("limited", limited), ("slow", slow)):
                before = len(RECORDS)
                result = execute(output / f"request-json.{language}", language, failure=True)
                assert len(RECORDS) == before + 1, (language, name)
                error = result.stdout + result.stderr
                assert ("Response size limit exceeded" in error) if name == "limited" else any(word in error.lower() for word in ("timeout", "deadline", "canceled", "cancelled")), error
                print(f"PASS {language} {name}", flush=True)
            for name, output in (("redirect", redirect), ("cross", cross)):
                before = len(RECORDS)
                execute(output / f"request-json.{language}", language)
                assert len(RECORDS) == before + 2, (language, name)
                validate("json", RECORDS[-1], api_key=False)
                headers = RECORDS[-1][2]
                if name == "redirect":
                    assert headers["authorization"] == "Bearer redirect-private"
                    assert headers["cookie"] == "session=redirect-private"
                else:
                    assert headers["authorization"] is None and headers["cookie"] is None
                print(f"PASS {language} {name}", flush=True)
    print(f"Validated {len(RECORDS)} additional generated HTTP requests")

try:
    main()
finally:
    for instance in SERVERS:
        instance.shutdown()
        instance.server_close()
