"""Allow List Object for Authentication."""

import json
import subprocess
from pathlib import Path

from .logger import get_logger

logger = get_logger(__name__)


class AllowList:
    """A simple allow list for IP addresses."""

    def __init__(self, allowlist_path: Path | None, nginx_allowlist_path: Path | None) -> None:
        """Initialize the allow list with a path to the allow list file."""
        self.allowlist_path = allowlist_path
        self.nginx_allowlist_path = nginx_allowlist_path
        self._nginx_bin_path: Path | None = _find_nginx_bin_path()

        self.allowlist_ips: list[str] = []
        self.load()
        self._ensure_correct_ips()
        self.save()

    def _ensure_correct_ips(self) -> None:
        """Fix any incorrect IP addresses in the allow list."""
        for ip in self.allowlist_ips:
            if ip.startswith("::ffff:"):
                ip_no_prefix = ip[7:]  # Remove IPv6 prefix
                if ip_no_prefix not in self.allowlist_ips:
                    self.allowlist_ips.append(ip_no_prefix)

    def add(self, ip: str) -> None:
        """Add an IP address to the allow list."""
        if ip == "":
            logger.warning("Attempted to add an empty IP address to the allow list.")
            return

        if ip not in self.allowlist_ips:
            self.allowlist_ips.append(ip)

            if ip.startswith("::ffff:"):
                self.allowlist_ips.append(ip[7:])  # Remove IPv6 prefix

            logger.info("Added IP address to allow list: %s", ip)
            self.save()

    def check(self, ip: str) -> bool:
        """Check if an IP address is in the allow list."""
        return ip in self.allowlist_ips

    def load(self) -> None:
        """Load the allow list from a file."""
        if not self.allowlist_path:
            return

        if self.allowlist_path.exists():
            with self.allowlist_path.open("r") as f:
                try:
                    self.allowlist_ips = json.load(f)
                except json.JSONDecodeError:
                    logger.error("Failed to decode JSON from allow list file, resetting")  # noqa: TRY400

    def save(self) -> None:
        """Save the allow list to a file."""
        if self.allowlist_path:
            with self.allowlist_path.open("w") as f:
                json.dump(self.allowlist_ips, f)

        if self.nginx_allowlist_path:
            with self.nginx_allowlist_path.open("w") as f:
                for ip in self.allowlist_ips:
                    f.write(f"allow {ip};\n")
                f.write("deny all;\n")
            logger.info("Nginx allow list updated with %d IPs", len(self.allowlist_ips))

            self._reload_nginx()

    def _reload_nginx(self) -> None:
        """Reload Nginx to apply the new allow list."""
        if not self._nginx_bin_path:
            logger.error("Nginx binary path not found, cannot reload Nginx.")
            return

        output = subprocess.run(  # noqa: S603 # I trust this subprocess call
            [self._nginx_bin_path, "-s", "reload"],
            check=True,
            capture_output=True,
            text=True,
            shell=False,
        )

        if output.returncode != 0:
            logger.error("Failed to reload Nginx: %s", output.stderr.strip())
            return


def _find_nginx_bin_path() -> Path | None:
    """Find the Nginx executable path."""
    possible_paths = [
        Path("/usr/sbin/nginx"),
        Path("/usr/local/sbin/nginx"),
        Path("/usr/bin/nginx"),
        Path("/usr/local/bin/nginx"),
    ]

    for path in possible_paths:
        if path.exists() and path.is_file():
            return path

    return None
