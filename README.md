# Stream proxy

`./scripts/grab-js.sh`

edit `Rocket.toml`

```bash
cargo run # Port 8999
cargo run --release # Port 443, requires certificates
```

## Certs

```bash
curl https://get.acme.sh | sh -s email=email@example.com
source ~/.acme.sh/acme.sh.env
acme.sh --set-default-ca --server letsencrypt
acme.sh --issue --standalone -d stream.example.com --fullchain-file private/fullchain.cer --key-file private/cert.key
```

## With Docker

Edit `docker-compose.yml` to set the correct domain and paths to certificates.

```bash
docker compose up -d
```
