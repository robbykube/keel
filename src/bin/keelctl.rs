use std::env;
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

struct Options {
    host: String,
    binary: PathBuf,
    service: PathBuf,
    admin_token: String,
    bind: String,
    port: u16,
}

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("bootstrap") => {
            let rest: Vec<String> = args.collect();
            match parse_options(&rest).and_then(bootstrap) {
                Ok(address) => println!("Keel is running: {address}"),
                Err(message) => {
                    eprintln!("keelctl: {message}");
                    std::process::exit(1);
                }
            }
        }
        Some("help") | Some("--help") | None => print_help(),
        Some(command) => {
            eprintln!("keelctl: unknown command `{command}`");
            print_help();
            std::process::exit(2);
        }
    }
}

fn print_help() {
    println!(
        "keelctl bootstrap --host user@machine --binary PATH --admin-token TOKEN [--bind IP] [--port PORT]"
    );
    println!("  Upload and install Keel on an existing Debian/Linux machine over SSH.");
    println!("  The remote user must be able to run sudo without an interactive password prompt.");
}

fn parse_options(args: &[String]) -> Result<Options, String> {
    let mut host = None;
    let mut binary = None;
    let mut service = None;
    let mut admin_token = None;
    let mut bind = "0.0.0.0".to_string();
    let mut port = 9478;

    let mut index = 0;
    while index < args.len() {
        let key = args[index].as_str();
        let value = |index: &mut usize| -> Result<String, String> {
            *index += 1;
            args.get(*index)
                .cloned()
                .ok_or_else(|| format!("missing value for {key}"))
        };
        match key {
            "--host" => host = Some(value(&mut index)?),
            "--binary" => binary = Some(PathBuf::from(value(&mut index)?)),
            "--service" => service = Some(PathBuf::from(value(&mut index)?)),
            "--admin-token" => admin_token = Some(value(&mut index)?),
            "--bind" => bind = value(&mut index)?,
            "--port" => {
                port = value(&mut index)?
                    .parse()
                    .map_err(|_| "port must be a number".to_string())?
            }
            "--help" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown option `{other}`")),
        }
        index += 1;
    }

    let host = host.ok_or("--host is required")?;
    let binary = binary.ok_or("--binary is required")?;
    let admin_token = admin_token.ok_or("--admin-token is required")?;
    validate_host(&host)?;
    validate_token(&admin_token)?;
    validate_bind(&bind)?;
    if port == 0 {
        return Err("port must be between 1 and 65535".into());
    }
    if !binary.is_file() {
        return Err(format!("binary does not exist: {}", binary.display()));
    }

    let service = service.unwrap_or_else(|| Path::new("packaging/systemd/keel.service").into());
    if !service.is_file() {
        return Err(format!(
            "service file does not exist: {}",
            service.display()
        ));
    }

    Ok(Options {
        host,
        binary,
        service,
        admin_token,
        bind,
        port,
    })
}

fn bootstrap(options: Options) -> Result<String, String> {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "clock is before Unix epoch")?
        .as_millis();
    let remote_binary = format!("/tmp/keel-{suffix}");
    let remote_service = format!("/tmp/keel-service-{suffix}");
    let remote_env = format!("/tmp/keel-env-{suffix}");

    scp(&options.binary, &options.host, &remote_binary)?;
    scp(&options.service, &options.host, &remote_service)?;
    let env_file = format!(
        "KEEL_ADMIN_TOKEN={}\nKEEL_BIND={}\nKEEL_PORT={}\n",
        options.admin_token, options.bind, options.port
    );
    ssh_stdin(
        &options.host,
        &format!("sudo install -m 600 /dev/stdin {remote_env}"),
        env_file.as_bytes(),
    )?;
    ssh(
        &options.host,
        &format!(
            "sudo install -D -m 0755 {remote_binary} /usr/bin/keel && \
             sudo install -D -m 0644 {remote_service} /etc/systemd/system/keel.service && \
             sudo install -D -m 0600 {remote_env} /etc/keel/keel.env && \
             sudo install -d -m 0700 /var/lib/keel && \
             sudo rm -f {remote_binary} {remote_service} {remote_env} && \
             sudo systemctl daemon-reload && sudo systemctl enable --now keel.service"
        ),
    )?;
    let address = ssh_capture(&options.host, "hostname -I")?
        .split_whitespace()
        .next()
        .unwrap_or(&options.host)
        .to_string();
    Ok(format!("http://{address}:{}", options.port))
}

fn scp(local: &Path, host: &str, remote: &str) -> Result<(), String> {
    let target = format!("{host}:{remote}");
    let status = Command::new("scp")
        .arg(local)
        .arg(target)
        .status()
        .map_err(|_| "could not start scp; install or enable OpenSSH on Windows".to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("scp failed for {}", local.display()))
    }
}

fn ssh(host: &str, command: &str) -> Result<(), String> {
    let status = Command::new("ssh")
        .arg(host)
        .arg(command)
        .status()
        .map_err(|_| "could not start ssh; install or enable OpenSSH on Windows".to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("remote bootstrap command failed".into())
    }
}

fn ssh_stdin(host: &str, command: &str, input: &[u8]) -> Result<(), String> {
    let mut child = Command::new("ssh")
        .arg(host)
        .arg(command)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|_| "could not start ssh; install or enable OpenSSH on Windows".to_string())?;
    child
        .stdin
        .take()
        .ok_or_else(|| "could not open ssh input".to_string())?
        .write_all(input)
        .map_err(|_| "could not send remote environment".to_string())?;
    let status = child.wait().map_err(|_| "ssh did not finish".to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("could not install remote environment".into())
    }
}

fn ssh_capture(host: &str, command: &str) -> Result<String, String> {
    let output = Command::new("ssh")
        .arg(host)
        .arg(command)
        .output()
        .map_err(|_| "could not start ssh".to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err("could not read the remote GUI address".into())
    }
}

fn validate_host(host: &str) -> Result<(), String> {
    if host.is_empty()
        || host.len() > 255
        || host
            .chars()
            .any(|character| character.is_whitespace() || "'\";|&<>`$".contains(character))
    {
        return Err("host must be a user@name-or-address without shell metacharacters".into());
    }
    Ok(())
}

fn validate_token(token: &str) -> Result<(), String> {
    if token.is_empty()
        || token
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || "._~-".contains(character)))
    {
        return Err(
            "admin token must use letters, numbers, dot, underscore, tilde, or dash".into(),
        );
    }
    Ok(())
}

fn validate_bind(bind: &str) -> Result<(), String> {
    if bind.parse::<IpAddr>().is_err() {
        return Err("bind must be an IPv4 or IPv6 address".into());
    }
    Ok(())
}
