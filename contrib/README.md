# Community Contributions (`contrib/`)

[Português](README.pt_BR.md) | **English**

This directory collects auxiliary scripts, infrastructure automation, and third-party tools contributed by the **SignalHunter** community.

The utilities located here are not part of the core daemon runtime, but provide streamlined deployment, monitoring integration, and operational workflows for external environments.

---

## Available Tools

### 1. `create_deploy.sh`
- **Author**: Alexandre Jeronimo Correa ([ajcorrea@gmail.com](mailto:ajcorrea@gmail.com))
- **Purpose**: Containerized deployment automation for SignalHunter using Docker Compose and MariaDB 11.
- **Key Technical Highlights**:
  - Multi-stage build producing a static binary targeting `x86_64-unknown-linux-musl`.
  - Minimal runtime container based on `debian:bookworm-slim` running under an unprivileged user (`UID/GID 1000`).
  - Automated high-entropy cryptographic secret generation for `master_encryption_key` (AES-256-GCM), `jwt_secret`, and MariaDB passwords using `openssl rand`.
  - MariaDB 11 container with active `healthcheck` ensuring database readiness before SignalHunter starts.
  - Configuration files and secrets stored with strict permissions (`chmod 600`).
- **Operation Modes**:
  - `./create_deploy.sh deploy`: First-time setup (prompts for web port, generates secrets, generates `compose.yaml`, builds Docker image, and starts services).
  - `./create_deploy.sh update`: Pulls latest changes from the `main` branch and rebuilds containers while preserving database volume.
  - `./create_deploy.sh clean`: Stops containers, removes local Docker image and volumes (requires explicit confirmation).
- **How to Use**:
  - Copy or download the script to the parent directory where you want to organize the deployment (e.g. `/opt/` or `/srv/`):
    ```bash
    cp contrib/create_deploy.sh /opt/deploy-signalhunter/
    cd /opt/deploy-signalhunter/
    ./create_deploy.sh deploy
    ```

---

## How to Contribute

If you developed a useful script, Ansible playbook, Prometheus exporter, Grafana dashboard, or webhook alert bridge:

1. Place your scripts and tools inside this `contrib/` directory.
2. Document the tool in both `README.md` and `README.pt_BR.md` including authorship, goals, dependencies, and usage instructions.
3. Open a Pull Request on the official repository.
