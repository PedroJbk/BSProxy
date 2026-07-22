use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use tokio::net::TcpStream;
use anyhow::Result;
use log::{info, debug};
use tokio::time::{timeout, Duration};

pub async fn handle_security(mut socket: TcpStream, status: &str) -> Result<()> {
    info!("🔐 SECURITY/SSHPRO handshake iniciado...");
    
    // 1. Resposta IMEDIATA: 101 (SSHPRO)
    // O Injetor muitas vezes espera o 101 antes de enviar o resto do payload.
    let resp1 = format!("HTTP/1.1 101 (SSHPRO) Informational\r\n\r\n");
    socket.write_all(resp1.as_bytes()).await?;
    debug!("📤 Sent Response 1: 101 (SSHPRO)");

    // 2. Tentar ler o payload, mas sem travar se ele não vier completo
    let mut buf = [0u8; 2048];
    let _ = timeout(Duration::from_millis(300), socket.read(&mut buf)).await;

    // 3. Segunda Resposta: 200 OK
    socket.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await?;
    debug!("📤 Sent Response 2: 200 OK");

    // 4. Terceira Resposta: 200 SSHPRO
    let resp3 = format!("HTTP/1.1 200 {}\r\n\r\n", if status.is_empty() { "SSHPRO" } else { status });
    socket.write_all(resp3.as_bytes()).await?;
    debug!("📤 Sent Response 3: 200 SSHPRO");

    info!("🔐 Handshake complete!");
    
    // Conectar ao backend SSH
    info!("🔗 Conectando ao backend SSH (127.0.0.1:22)...");
    let mut remote = match TcpStream::connect("127.0.0.1:22").await {
        Ok(s) => s,
        Err(e) => {
            info!("⚠️ SSH falhou ({}), tentando VPN (127.0.0.1:1194)...", e);
            TcpStream::connect("127.0.0.1:1194").await?
        }
    };

    info!("✅ Túnel iniciado.");
    let _ = copy_bidirectional(&mut socket, &mut remote).await;
    
    Ok(())
}
