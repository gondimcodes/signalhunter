# Contribuições da Comunidade (`contrib/`)

**Português** | [English](README.md)

Este diretório reúne scripts auxiliares, automações de infraestrutura e ferramentas complementares desenvolvidas por colaboradores e membros da comunidade do **SignalHunter**.

Os utilitários aqui presentes não fazem parte do runtime central do sistema, mas facilitam implantações, integrações e rotinas operacionais em ambientes externos.

---

## Índice de Ferramentas

### 1. `create_deploy.sh`
- **Autor**: Alexandre Jeronimo Correa ([ajcorrea@gmail.com](mailto:ajcorrea@gmail.com))
- **Objetivo**: Automação de deploy containerizado para o SignalHunter utilizando Docker Compose e MariaDB 11.
- **Destaques Técnicos**:
  - Compilação estática do binário Rust com a target `x86_64-unknown-linux-musl` em multi-stage build.
  - Imagem de runtime mínima (`debian:bookworm-slim`) executando com usuário isolado e sem privilégios (`UID/GID 1000`).
  - Geração automática e aleatória de chaves de alta entropia para `master_encryption_key` (AES-256-GCM), `jwt_secret` e senhas do MariaDB com `openssl rand`.
  - Configuração do MariaDB 11 com `healthcheck` garantindo que o banco esteja pronto antes do SignalHunter subir.
  - Arquivos de configuração e senhas gravados com permissão restrita (`chmod 600`).
- **Modos de Operação**:
  - `./create_deploy.sh deploy`: Realiza a primeira instalação completa (pergunta a porta web desejada, gera credenciais, cria `compose.yaml`, compila a imagem Docker e inicializa os containers).
  - `./create_deploy.sh update`: Sincroniza o código-fonte com a branch `main` do GitHub e reconstrói os containers preservando o banco de dados.
  - `./create_deploy.sh clean`: Para os containers, remove a imagem local e os volumes Docker (com confirmação obrigatória).
- **Como Utilizar**:
  - Recomenda-se copiar ou baixar o script para o diretório pai onde você deseja estruturar o deploy (ex: `/opt/` ou `/srv/`):
    ```bash
    cp contrib/create_deploy.sh /opt/deploy-signalhunter/
    cd /opt/deploy-signalhunter/
    ./create_deploy.sh deploy
    ```

---

## Como Contribuir

Se você desenvolveu um script útil, playbook Ansible, exporter para Prometheus, template para Grafana ou integração de alertas que possa beneficiar outros provedores e operadores:

1. Coloque seus arquivos e scripts dentro deste diretório `contrib/`.
2. Documente no `README.md` e `README.pt_BR.md` a autoria, objetivos, dependências e modo de uso.
3. Abra um Pull Request no repositório oficial com a sua contribuição.
