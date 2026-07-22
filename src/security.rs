use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use tokio::net::TcpStream;
use anyhow::Result;
use log::info;

pub async fn handle_security(mut socket: TcpStream) -> Result<()> {
    info!("🔐 SECURITY handshake...");
    
    let mut buf = [0u8; 1024];
    let n = socket.read(&mut buf).await?;
    let data = String::from_utf8_lossy(&buf[..n]);
    
    info!("📩 SECURITY: {}", data);
    
    // Resposta solicitada: 200 OK + headers de upgrade
    let response = "HTTP/1.1 200 OK\r\n\
                    Connection: Upgrade\r\n\
                    Upgrade: security\r\n\
                    \r\n";
    
    socket.write_all(response.as_bytes()).await?;
    info!("🔐 SECURITY handshake complete!");
    
    // Encaminhar para SSH local
    match TcpStream::connect("127.0.0.1:22").await {
        Ok(mut remote) => {
            info!("✅ SECURITY -> SSH conectado");
            let _ = copy_bidirectional(&mut socket, &mut remote).await;
            info!("🔚 Conexão SECURITY->SSH encerrada");
            Ok(())
        }
        Err(e) => {
            info!("❌ Falha ao conectar ao SSH: {}", e);
            // Tentar VPN local como fallback
            match TcpStream::connect("127.0.0.1:1194").await {
                Ok(mut remote) => {
                    info!("✅ SECURITY -> VPN conectado");
                    let _ = copy_bidirectional(&mut socket, &mut remote).await;
                    Ok(())
                }
                Err(e2) => {
                    anyhow::bail!("Security connection failed: SSH={}, VPN={}", e, e2)
                }
            }
        }
    }
}
