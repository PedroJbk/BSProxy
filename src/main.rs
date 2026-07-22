use std::env;
use std::io::Error;
use tokio::io::copy_bidirectional;
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};

mod socks5;
mod websocket;
mod security;
mod tcp_fallback;
mod tls;

#[tokio::main]
async fn main() -> Result<(), Error> {
    // Inicializa o logger com nível INFO por padrão
    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "info");
    }
    env_logger::init();

    let args: Vec<String> = env::args().collect();
    let config = parse_args(&args);

    let port = config.port;
    let status = config.status.clone();
    let use_tls = config.tls;
    let ssh_only = config.ssh_only;

    log::info!("🚀 AWProxy iniciando...");
    log::info!("📡 Porta: {}, Status: '{}', TLS: {}, SSH-Only: {}", port, status, use_tls, ssh_only);

    let listener = TcpListener::bind(format!("[::]:{}", port)).await?;
    println!("Servidor iniciado na porta: {}", port);

    start_proxy(listener, status, ssh_only, use_tls).await;
    Ok(())
}

async fn start_proxy(listener: TcpListener, status: String, ssh_only: bool, use_tls: bool) {
    loop {
        let status_clone = status.clone();
        match listener.accept().await {
            Ok((client_stream, addr)) => {
                log::info!("📥 Nova conexão de: {}", addr);
                tokio::spawn(async move {
                    if let Err(e) = handle_client(client_stream, &status_clone, ssh_only, use_tls).await {
                        eprintln!("❌ Erro ao processar cliente {}: {}", addr, e);
                    }
                });
            }
            Err(e) => eprintln!("Erro ao aceitar conexão: {}", e),
        }
    }
}

async fn handle_client(mut client_stream: TcpStream, status: &str, ssh_only: bool, use_tls: bool) -> Result<(), Error> {

    if use_tls {
        log::info!("🔒 Modo TLS ativado");
        return tls::handle_tls(client_stream).await.map_err(|e| Error::new(std::io::ErrorKind::Other, e));
    }

    if ssh_only {
        // Aplica a lógica de Tripla Resposta mesmo em SSH Only se for HTTP
        log::info!("🔐 Modo SSH-Only ativado");
        let mut buffer = [0u8; 4096];
        let bytes_peeked = match timeout(Duration::from_millis(500), client_stream.peek(&mut buffer)).await {
            Ok(Ok(n)) => n,
            _ => 0,
        };

        if bytes_peeked > 0 {
            let data = String::from_utf8_lossy(&buffer[..bytes_peeked]);
            let data_upper = data.to_uppercase();

            // Detecção agressiva de SECURITY no modo SSH-Only
            if is_security_request(&data_upper) {
                log::info!("🔍 SSH-Only: SECURITY detectado!");
                return security::handle_security(client_stream, status).await.map_err(|e| Error::new(std::io::ErrorKind::Other, e));
            }

            if is_http_request(&data_upper) {
                log::info!("🌐 SSH-Only: HTTP/WebSocket detectado");
                return websocket::handle_websocket(client_stream, status).await.map_err(|e| Error::new(std::io::ErrorKind::Other, e));
            }

            // Se começar com SSH-
            if data.contains("SSH-") {
                log::info!("🔗 SSH-Only: Conexão SSH direta detectada");
            }
        }

        // Se não for HTTP, faz o túnel direto
        let mut server_stream = match TcpStream::connect("127.0.0.1:22").await {
            Ok(s) => s,
            Err(_) => return Ok(()),
        };
        let _ = copy_bidirectional(&mut client_stream, &mut server_stream).await;
        return Ok(());
    }

    // Espiada rápida no buffer (Peek)
    let mut buffer = [0u8; 4096];
    let bytes_read = match timeout(Duration::from_millis(500), client_stream.peek(&mut buffer)).await {
        Ok(Ok(n)) => n,
        _ => 0,
    };

    if bytes_read > 0 {
        let first_byte = buffer[0];
        let data = String::from_utf8_lossy(&buffer[..bytes_read]);
        let data_upper = data.to_uppercase();

        log::debug!("🔍 Peek ({} bytes): {:?}", bytes_read, &data[..std::cmp::min(bytes_read, 300)]);

        // 1. SOCKS5
        if first_byte == 0x05 {
            log::info!("🔐 SOCKS5 detectado");
            return socks5::handle_socks5(client_stream).await.map_err(|e| Error::new(std::io::ErrorKind::Other, e));
        }

        // 2. TLS/SSL Handshake (0x16)
        if first_byte == 0x16 {
            log::info!("🔒 TLS/SSL Handshake detectado");
            return tls::handle_tls(client_stream).await.map_err(|e| Error::new(std::io::ErrorKind::Other, e));
        }

        // 3. HTTP / WebSocket / Custom Methods
        if is_http_request(&data_upper) {
            log::info!("🌐 HTTP request detectado");

            // === DETECÇÃO AGRESSIVA DE SECURITY ===
            if is_security_request(&data_upper) {
                log::info!("🔐 *** MODO SECURITY ATIVADO ***");
                return security::handle_security(client_stream, status).await.map_err(|e| Error::new(std::io::ErrorKind::Other, e));
            }

            // WebSocket padrão (Tripla Resposta)
            log::info!("🌐 WebSocket Tripla Resposta");
            return websocket::handle_websocket(client_stream, status).await.map_err(|e| Error::new(std::io::ErrorKind::Other, e));
        }

        // 4. Dados não-HTTP mas com padrões especiais (ex: headers customizados)
        if is_security_request(&data_upper) {
            log::info!("🔐 *** MODO SECURITY ATIVADO (não-HTTP) ***");
            return security::handle_security(client_stream, status).await.map_err(|e| Error::new(std::io::ErrorKind::Other, e));
        }
    }

    // Fallback: TCP puro
    log::info!("📡 Fallback para TCP puro");
    tcp_fallback::handle_tcp(client_stream).await.map_err(|e| Error::new(std::io::ErrorKind::Other, e))
}

/// Detecção ultra-agressiva de requisições SECURITY
/// Cobre todos os cenários possíveis: method, header, valor, etc.
fn is_security_request(data_upper: &str) -> bool {
    // Métodos HTTP customizados
    if data_upper.starts_with("SECURITY") ||
       data_upper.starts_with("ACL") ||
       data_upper.starts_with("PATCH") ||
       data_upper.starts_with("PROPFIND") ||
       data_upper.starts_with("PROPPATCH") ||
       data_upper.starts_with("MKCALENDAR") ||
       data_upper.starts_with("REPORT") {
        return true;
    }

    // Headers com SECURITY
    if data_upper.contains("SECURITY") ||
       data_upper.contains("X-SECURITY") ||
       data_upper.contains("UPGRADE: SECURITY") ||
       data_upper.contains("UPGRADE:SECURITY") {
        return true;
    }

    // Headers especiais comuns em proxies HTTP
    if data_upper.contains("X-INJECT") ||
       data_upper.contains("X-CUSTOM") ||
       data_upper.contains("X-PROXY") ||
       data_upper.contains("HTTP-PROXY") ||
       data_upper.contains("HTTP-CONNECT") {
        return true;
    }

    // Verificar se é um método customizado (palavra maiúscula seguida de espaço e /)
    // Ex: "ACL / HTTP/1.1" ou "SECURITY / HTTP/1.1"
    if let Some(space_pos) = data_upper.find(' ') {
        let method = &data_upper[..space_pos];
        let methods_custom = ["SECURITY", "ACL", "PATCH", "PROPFIND", "PROPPATCH", "MKCALENDAR", "REPORT", "SEARCH"];
        if methods_custom.contains(&method) {
            return true;
        }
    }

    false
}

fn is_http_request(data_upper: &str) -> bool {
    let methods = ["GET", "POST", "PUT", "DELETE", "CONNECT", "OPTIONS", "HEAD", "TRACE"];
    for m in methods {
        if data_upper.contains(m) { return true; }
    }
    data_upper.contains("HTTP/1.") || data_upper.contains("HTTP/2.")
}

struct ProxyConfig {
    port: u16,
    status: String,
    tls: bool,
    ssh_only: bool,
}

fn parse_args(args: &[String]) -> ProxyConfig {
    let mut port = 80u16;
    let mut status = "200 OK".to_string();
    let mut tls = false;
    let mut ssh_only = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-p" => { if i+1 < args.len() { port = args[i+1].parse().unwrap_or(80); i+=1; } }
            "-s" => { if i+1 < args.len() { status = args[i+1].clone(); i+=1; } }
            "-t" => { tls = true; }
            "-ssh" => { ssh_only = true; }
            _ => {}
        }
        i += 1;
    }
    ProxyConfig { port, status, tls, ssh_only }
}
