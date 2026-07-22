use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use tokio::net::TcpStream;
use anyhow::Result;
use log::{info, debug};

pub async fn handle_security(mut socket: TcpStream, status: &str) -> Result<()> {
    info!("🔐 SECURITY handshake (Tripla Resposta)...");
    
    // Consumir os headers da requisição inicial de forma segura
    let mut buf = [0u8; 4096];
    let n = socket.read(&mut buf).await?;
    if n > 0 {
        debug!("📥 SECURITY Request: {}", String::from_utf8_lossy(&buf[..n]));
    }
    
    // Conforme o print do usuário e requisitos de "TCP/SECURITY":
    
    // 1. Status: 101 (STATUS) Informational
    let resp1 = format!("HTTP/1.1 101 TCP/SECURITY {}\r\n\r\n", status);
    socket.write_all(resp1.as_bytes()).await?;
    debug!("📤 Sent Response 1: 101 TCP/SECURITY");

    // 2. Enviando 200 HTTP status - HTTP/1.1 200 OK
    socket.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;
    debug!("📤 Sent Response 2: 200 OK");

    // 3. HTTP/1.1 200 (STATUS)
    let resp3 = format!("HTTP/1.1 200 TCP/SECURITY {}\r\n\r\n", status);
    socket.write_all(resp3.as_bytes()).await?;
    debug!("📤 Sent Response 3: 200 TCP/SECURITY");

    info!("🔐 SECURITY handshake complete!");
    
    // Conectar ao backend (SSH por padrão para Security)
    info!("🔗 Conectando ao backend SSH (127.0.0.1:22)...");
    let mut remote = match TcpStream::connect("127.0.0.1:22").await {
        Ok(s) => s,
        Err(e) => {
            info!("⚠️ SSH falhou ({}), tentando VPN (127.0.0.1:1194)...", e);
            TcpStream::connect("127.0.0.1:1194").await?
        }
    };

    info!("✅ SECURITY Túnel iniciado.");
    let _ = copy_bidirectional(&mut socket, &mut remote).await;
    
    Ok(())
}
