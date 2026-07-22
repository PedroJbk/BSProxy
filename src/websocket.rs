use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use tokio::net::TcpStream;
use anyhow::Result;
use log::info;
use tokio::time::{timeout, Duration};

/// Consome os headers de forma segura sem travar
async fn consume_http_headers(socket: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = [0u8; 1];
    let mut consecutive_newlines = 0;

    while consecutive_newlines < 2 {
        let n = socket.read(&mut buf).await?;
        if n == 0 { break; }
        
        if buf[0] == b'\n' {
            consecutive_newlines += 1;
        } else if buf[0] != b'\r' {
            consecutive_newlines = 0;
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
        log::debug!("📥 WebSocket Request: {}", data);
    }

    // Consumir os headers da requisição
    let _ = consume_http_headers(&mut socket).await;
    
    // 1. Primeira Resposta: 101
    socket.write_all(format!("HTTP/1.1 101 {}\r\n\r\n", status).as_bytes()).await?;

    // 2. Leitura intermediária (conforme o código funcional do usuário)
    let mut buffer = [0; 1024];
    let _ = socket.read(&mut buffer).await?;

    // 3. Segunda Resposta: 101
    socket.write_all(format!("HTTP/1.1 101 {}\r\n\r\n", status).as_bytes()).await?;

    // 4. Terceira Resposta: 200
    socket.write_all(format!("HTTP/1.1 200 {}\r\n\r\n", status).as_bytes()).await?;

    // Detecção de protocolo (SSH vs VPN) usando Peek
    let addr_proxy = match timeout(Duration::from_secs(5), peek_stream(&socket)).await {
        Ok(Ok(data)) if data.contains("SSH") || data.is_empty() => "127.0.0.1:22",
        Ok(_) => "127.0.0.1:1194",
        Err(_) => "127.0.0.1:22",
    };

    info!("🔗 Conectando ao backend: {}", addr_proxy);

    // Conectar ao backend detectado
    let mut remote = match TcpStream::connect(addr_proxy).await {
        Ok(s) => s,
        Err(_) => {
            // Fallback se a detecção falhar ou o serviço estiver offline
            TcpStream::connect("127.0.0.1:22").await?
        }
    };

    info!("✅ Túnel iniciado.");
    
    // O segredo para não travar: copy_bidirectional
    let _ = copy_bidirectional(&mut socket, &mut remote).await;
    
    Ok(())
}
