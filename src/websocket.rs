use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use tokio::net::TcpStream;
use anyhow::Result;
use log::info;
use tokio::time::{timeout, Duration};

/// Consome os headers de forma segura sem travar
async fn consume_http_headers(socket: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = [0u8; 1];
    let mut consecutive_newlines = 0;

    loop {
        match timeout(Duration::from_millis(300), socket.read(&mut buf)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(_)) => {
                if buf[0] == b'\n' {
                    consecutive_newlines += 1;
                    if consecutive_newlines >= 2 {
                        break;
                    }
                } else if buf[0] != b'\r' {
                    consecutive_newlines = 0;
                }
            }
            Ok(Err(_)) => break,
            Err(_) => break, // timeout - seguir mesmo assim
        }
    }
    Ok(())
}

async fn peek_stream(stream: &TcpStream) -> std::io::Result<String> {
    let mut buffer = vec![0; 8192];
    let bytes_peeked = stream.peek(&mut buffer).await?;
    Ok(String::from_utf8_lossy(&buffer[..bytes_peeked]).to_string())
}

pub async fn handle_websocket(mut socket: TcpStream, status: &str) -> Result<()> {
    info!("🌐 WebSocket Tripla Resposta Handshake...");

    // Espiar headers para log
    if let Ok(data) = peek_stream(&socket).await {
        log::debug!("📥 WebSocket Request: {}", data.trim());
    }

    // Consumir os headers da requisição (sem bloquear)
    let _ = consume_http_headers(&mut socket).await;

    // 1. Primeira Resposta: 101
    socket.write_all(format!("HTTP/1.1 101 Switching Protocols\r\n\
                              Upgrade: websocket\r\n\
                              Connection: Upgrade\r\n\
                              Status: {}\r\n\
                              \r\n", status).as_bytes()).await?;
    socket.flush().await?;
    info!("📤 WebSocket Resposta 1: 101 Switching Protocols");

    // 2. Leitura intermediária
    tokio::time::sleep(Duration::from_millis(50)).await;
    let mut buffer = [0u8; 1024];
    let _ = timeout(Duration::from_millis(200), socket.read(&mut buffer)).await;

    // 3. Segunda Resposta: 101
    socket.write_all(format!("HTTP/1.1 101 Switching Protocols\r\n\
                              Upgrade: websocket\r\n\
                              Connection: Upgrade\r\n\
                              \r\n").as_bytes()).await?;
    socket.flush().await?;
    info!("📤 WebSocket Resposta 2: 101 Switching Protocols");

    // 4. Terceira Resposta: 200
    tokio::time::sleep(Duration::from_millis(50)).await;
    socket.write_all(format!("HTTP/1.1 200 {}\r\n\
                              Connection: keep-alive\r\n\
                              \r\n", status).as_bytes()).await?;
    socket.flush().await?;
    info!("📤 WebSocket Resposta 3: 200 {}", status);

    // Detecção de protocolo (SSH vs VPN) usando Peek
    let addr_proxy = match timeout(Duration::from_secs(5), peek_stream(&socket)).await {
        Ok(Ok(data)) if data.contains("SSH") || data.starts_with("SSH-") => "127.0.0.1:22",
        Ok(Ok(data)) if data.is_empty() => "127.0.0.1:22",
        Ok(_) => "127.0.0.1:1194",
        Err(_) => "127.0.0.1:22",
    };

    info!("🔗 Conectando ao backend: {}", addr_proxy);

    // Conectar ao backend detectado
    let mut remote = match TcpStream::connect(addr_proxy).await {
        Ok(s) => s,
        Err(e) => {
            log::warn!("⚠️ Falha ao conectar em {}: {}, tentando SSH", addr_proxy, e);
            TcpStream::connect("127.0.0.1:22").await?
        }
    };

    info!("✅ WebSocket Túnel iniciado.");
    let _ = copy_bidirectional(&mut socket, &mut remote).await;
    info!("🔚 WebSocket Túnel finalizado.");

    Ok(())
}
