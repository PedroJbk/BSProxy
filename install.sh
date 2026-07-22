#!/bin/bash
# AWProxy Installer - Fixed Version
REPO_URL="https://github.com/PedroJbk/AWProxy.git"
REPO_BRANCH="main"
CMD_NAME="awproxy"
TOTAL_STEPS=9
CURRENT_STEP=0

show_progress() {
    PERCENT=$((CURRENT_STEP * 100 / TOTAL_STEPS))
    echo -e "\033[1;34mProgresso: [${PERCENT}%] - $1\033[0m"
}

error_exit() {
    echo -e "\n\033[1;31mErro: $1\033[0m"
    exit 1
}

increment_step() {
    CURRENT_STEP=$((CURRENT_STEP + 1))
}

if [ "$EUID" -ne 0 ]; then
    error_exit "EXECUTE COMO ROOT (sudo su)"
fi

clear
echo -e "\033[0;34m    █████╗ ██╗    ██╗██████╗ ██████╗  ██████╗ ██╗  ██╗██╗   ██╗"
echo -e "\033[0;37m   ██╔══██╗██║    ██║██╔══██╗██╔══██╗██╔═══██╗╚██╗██╔╝╚██╗ ██╔╝"
echo -e "\033[0;34m   ███████║██║ █╗ ██║██████╔╝██████╔╝██║   ██║ ╚███╔╝  ╚████╔╝ "
echo -e "\033[0;37m   ██╔══██║██║███╗██║██╔═══╝ ██╔══██╗██║   ██║ ██╔██╗   ╚██╔╝  "
echo -e "\033[0;34m   ██║  ██║╚███╔███╔╝██║     ██║  ██║╚██████╔╝██╔╝ ██╗   ██║   "
echo -e "\033[0;37m   ╚═╝  ╚═╝ ╚══╝╚══╝ ╚═╝     ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝   ╚═╝   "
echo -e "\033[0;34m--------------------------------------------------------------\033[0m"

show_progress "Atualizando repositorios e dependencias..."
apt update -y > /dev/null 2>&1
apt install curl build-essential git lsb-release libssl-dev pkg-config -y > /dev/null 2>&1 || error_exit "Falha ao instalar pacotes"
increment_step

show_progress "Verificando o sistema..."
increment_step

show_progress "Instalando Rust (AWPro)..."
if ! command -v rustc &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y > /dev/null 2>&1
    source "$HOME/.cargo/env"
fi
export PATH="$HOME/.cargo/bin:$PATH"
increment_step

show_progress "Baixando AWProxy do GitHub..."
rm -rf /root/AWProxy
git clone --branch "$REPO_BRANCH" "$REPO_URL" /root/AWProxy > /dev/null 2>&1 || error_exit "Falha ao clonar"
increment_step

show_progress "Compilando (isso pode levar 2-5 minutos)..."
cd /root/AWProxy
cargo build --release || error_exit "Falha na compilação. Verifique as dependências."
increment_step

show_progress "Instalando binários..."
mkdir -p /opt/awproxy
cp ./target/release/awproxy /opt/awproxy/proxy
chmod +x /opt/awproxy/proxy
if [ -f "menu.sh" ]; then
    cp menu.sh /opt/awproxy/menu
    chmod +x /opt/awproxy/menu
    ln -sf /opt/awproxy/menu /usr/local/bin/awproxy
else
    ln -sf /opt/awproxy/proxy /usr/local/bin/awproxy
fi
increment_step

show_progress "Limpando temporários..."
rm -rf /root/AWProxy
increment_step

echo -e "\033[0;32m✅ Instalação concluída! Digite 'awproxy' para iniciar.\033[0m"
