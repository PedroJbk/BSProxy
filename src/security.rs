use tokio::io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional};
use tokio::net::TcpStream;
use anyhow::Result;
use log::{info, debug, warn};
use tokio::time::{timeout, Duration};

pub async fn handle_security(mut socket: TcpStream, status: &str) -> Result<()> {
    info!("🔐 SECURITY handshake (Tripla Resposta)...");

    // ============================================================
    // ETAPA 1: Consumir os headers da requisição ANTES de responder
    // Para não travar a leitura, usamos um timeout curto
    // ============================================================
    let mut buf = vec![0u8; 4096];
    let mut total_read = 0usize;
    let mut consecutive_newlines = 0u32;

    loop {
        match timeout(Duration::from_millis(300), socket.read(&mut buf[total_read..])).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                total_read += n;
                // Verificar se já lemos o fim dos headers (\r\n\r\n)
                for i in total_read.saturating_sub(4)..total_read {
                    if buf[i] == b'\n' {
                        consecutive_newlines += 1;
                        if consecutive_newlines >= 2 {
                            break;
                        }
                    } else if buf[i] != b'\r' {
                        consecutive_newlines = 0;
                    }
                }
                if consecutive_newlines >= 2 {
                    break;
                }
                if total_read >= 4096 {
                    break;
                }
            }
            Ok(Err(_)) => break,
            Err(_) => break, // timeout - headers incompletos, seguir mesmo assim
        }
    }

    if total_read > 0 {
        let data = String::from_utf8_lossy(&buf[..total_read]);
        debug!("📥 SECURITY Request ({} bytes): {}", total_read, data.trim());
    }

    // ============================================================
    // ETAPA 2: Primeira Resposta - 101 Switching Protocols
    // ============================================================
    let response1 = format!("HTTP/1.1 101 Switching Protocols\r\n\
                             Upgrade: security\r\n\
                             Connection: Upgrade\r\n\
                             Status: {}\r\n\
                             \r\n", status);
    socket.write_all(response1.as_bytes()).await?;
    socket.flush().await?;
    info!("📤 Resposta 1 enviada: 101 Switching Protocols (status: {})", status);

    // ============================================================
    // ETAPA 3: Segunda Resposta - 200 OK com Upgrade
    // Pequeno delay para simular handshake real
    // ============================================================
    tokio::time::sleep(Duration::from_millis(50)).await;

    let response2 = format!("HTTP/1.1 200 OK\r\n\
                             Connection: Upgrade\r\n\
                             Upgrade: security\r\n\
                             Status: {}\r\n\
                             \r\n", status);
    socket.write_all(response2.as_bytes()).await?;
    socket.flush().await?;
    info!("📤 Resposta 2 enviada: 200 OK (Upgrade: security, status: {})", status);

    // ============================================================
    // ETAPA 4: Terceira Resposta - 200 OK final com status
    // ============================================================
    tokio::time::sleep(Duration::from_millis(50)).await;

    let response3 = format!("HTTP/1.1 200 {}\r\n\
                             Content-Type: text/plain\r\n\
                             Connection: keep-alive\r\n\
                             \r\n", status);
    socket.write_all(response3.as_bytes()).await?;
    socket.flush().await?;
    info!("📤 Resposta 3 enviada: 200 {} (final)", status);

    info!("🔐 SECURITY handshake completo! (3 respostas enviadas)");

    // ============================================================
    // ETAPA 5: Detecção de backend (SSH vs VPN)
    // ============================================================
    let mut peek_buffer = [0u8; 1024];
    let addr_proxy = match timeout(Duration::from_millis(500), socket.peek(&mut peek_buffer)).await {
        Ok(Ok(n)) if n > 0 => {
            let data = String::from_utf8_lossy(&peek_buffer[..n]);
            debug!("🔍 Peek SECURITY ({} bytes): {:?}", n, &data[..std::cmp::min(n, 200)]);
            if data.contains("SSH") || data.starts_with("SSH-") {
                "127.0.0.1:22"
            } else {
                "127.0.0.1:1194"
            }
        }
        _ => {
            info!("⚠️ Peek timeout/vazio, usando SSH fallback");
            "127.0.0.1:22"
        }
    };

    info!("🔗 SECURITY -> Conectando ao backend: {}", addr_proxy);

    // ============================================================
    // ETAPA 6: Túnel bidirecional
    // ============================================================
    let mut remote = match TcpStream::connect(addr_proxy).await {
        Ok(s) => s,
        Err(e) => {
            warn!("❌ Falha ao conectar em {}: {}, tentando SSH fallback", addr_proxy, e);
            TcpStream::connect("127.0.0.1:22").await?
        }
    };

    info!("✅ SECURITY Túnel iniciado!");
    let _ = copy_bidirectional(&mut socket, &mut remote).await;
    info!("🔚 SECURITY Túnel finalizado.");

    Ok(())
}
