#!/usr/bin/env python3
"""
Hermes Windows Runtime Bundler
Downloads embeddable Python, installs Hermes Agent + dependencies,
and packages everything for distribution with the NSIS installer.

Usage:
    python scripts/bundle-runtime.py [--python-version 3.11.9] [--arch win64]

Run this script during CI (or locally) before building the Tauri app.
Output: src-tauri/bundle/hermes-runtime/
"""

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
RUNTIME_DIR = REPO_ROOT / "src-tauri" / "bundle" / "hermes-runtime"
PYTHON_DIR = RUNTIME_DIR / "python"
AGENT_DIR = RUNTIME_DIR / "hermes-agent"

# Windows embeddable Python download URLs
PYTHON_DOWNLOADS = {
    "3.11.9": {
        "win64": "https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-amd64.zip",
        "win32": "https://www.python.org/ftp/python/3.11.9/python-3.11.9-embed-win32.zip",
    },
    "3.12.8": {
        "win64": "https://www.python.org/ftp/python/3.12.8/python-3.12.8-embed-amd64.zip",
        "win32": "https://www.python.org/ftp/python/3.12.8/python-3.12.8-embed-win32.zip",
    },
}


def parse_args():
    parser = argparse.ArgumentParser(description="Bundle Hermes Windows runtime")
    parser.add_argument("--python-version", default="3.11.9", help="Python version to bundle")
    parser.add_argument("--arch", default="win64", choices=["win64", "win32"], help="Architecture")
    parser.add_argument("--hermes-source", help="Path to Hermes Agent source (default: ~/hermes-agent)")
    parser.add_argument("--skip-download", action="store_true", help="Skip Python download if cached")
    return parser.parse_args()


def download_file(url, dest):
    """Download a file with progress indicator."""
    print(f"  Downloading {url.split('/')[-1]}...")
    urllib.request.urlretrieve(url, dest)
    print(f"  -> {dest}")


def setup_embeddable_python(python_zip_path, python_dir):
    """Extract and configure embeddable Python."""
    print("\n[1/4] Extracting embeddable Python...")
    python_dir.mkdir(parents=True, exist_ok=True)

    with zipfile.ZipFile(python_zip_path, "r") as zf:
        zf.extractall(python_dir)

    # Enable pip by editing python*._pth
    # Rename the file and add "import site" to enable pip
    pth_files = list(python_dir.glob("python*._pth"))
    if not pth_files:
        print("  WARNING: No ._pth file found. Trying to enable pip manually.")
    else:
        pth = pth_files[0]
        content = pth.read_text()
        if "#import site" in content:
            content = content.replace("#import site", "import site")
            pth.write_text(content)
            print("  Enabled 'import site' in ._pth for pip support.")

    # Download get-pip.py and install pip
    print("\n[2/4] Installing pip...")
    python_exe = python_dir / "python.exe"
    if not python_exe.exists():
        print(f"  ERROR: python.exe not found in {python_dir}")
        sys.exit(1)

    get_pip = python_dir / "get-pip.py"
    download_file("https://bootstrap.pypa.io/get-pip.py", get_pip)

    subprocess.run([str(python_exe), str(get_pip), "--no-warn-script-location"],
                   check=True, capture_output=True)
    get_pip.unlink()
    print("  pip installed.")


def install_dependencies(python_dir):
    """Install Hermes Agent dependencies."""
    print("\n[3/4] Installing Hermes Agent dependencies...")
    python_exe = python_dir / "python.exe"

    # Core dependencies
    deps = [
        "httpx",
        "httpx-sse",
        "rich",
        "pyyaml",
        "Pillow",
        "requests",
        "python-dotenv",
        "jinja2",
        "aiohttp",
        "websockets",
        "pydantic",
        "pyautogen-core",
    ]

    for dep in deps:
        print(f"  Installing {dep}...")
        subprocess.run(
            [str(python_exe), "-m", "pip", "install", dep, "--no-warn-script-location"],
            check=True, capture_output=True,
        )

    print("  Dependencies installed.")


def copy_hermes_agent(hermes_source, agent_dir):
    """Copy Hermes Agent source files."""
    print("\n[4/4] Copying Hermes Agent source...")
    hermes_src = Path(hermes_source)
    if not hermes_src.exists():
        print(f"  WARNING: Hermes Agent source not found at {hermes_src}")
        print(f"  Create a placeholder or copy manually.")
        agent_dir.mkdir(parents=True, exist_ok=True)
        (agent_dir / "hermes").write_text("#!/usr/bin/env python3\n# Hermes Agent placeholder\n")
        return

    agent_dir.mkdir(parents=True, exist_ok=True)

    # Copy core files
    items_to_copy = [
        "hermes",
        "cli.py",
        "run_agent.py",
        "hermes_constants.py",
        "hermes_logging.py",
        "hermes_state.py",
        "hermes_time.py",
        "utils.py",
        "toolsets.py",
        "toolset_distributions.py",
        "model_tools.py",
        "mcp_serve.py",
        "trajectory_compressor.py",
        "batch_runner.py",
        "mini_swe_runner.py",
        "hermes_cli/",
        "agent/",
        "tools/",
        "skills/",
        "plugins/",
        "environments/",
        "gateway/",
        "acp_adapter/",
        "acp_registry/",
        "cron/",
    ]

    for item in items_to_copy:
        src = hermes_src / item
        dst = agent_dir / item
        if src.is_dir():
            if src.exists():
                shutil.copytree(src, dst, dirs_exist_ok=True,
                                ignore=shutil.ignore_patterns("__pycache__", "*.pyc", ".git"))
                print(f"  Copied {item}")
        elif src.exists():
            shutil.copy2(src, dst)
            print(f"  Copied {item}")

    # Create minimal config
    config_file = agent_dir / "cli-config.yaml"
    if not config_file.exists():
        config_file.write_text("""# Hermes Agent CLI Config (auto-generated for Hermes Windows)
llm:
  provider: minimax
  model: MiniMax-Text-01
  api_key_env: MINIMAX_API_KEY
mode: cli
tools:
  enabled: ["*"]
skills:
  enabled: ["*"]
plugins:
  enabled: ["*"]
logging:
  level: WARNING
""")
        print("  Created default cli-config.yaml")

    print("  Hermes Agent source copied.")


def enable_zip_import(python_dir):
    """Ensure python can import .zip stdlib (needed for embeddable Python)."""
    # Add the python3X.zip back if removed, and ensure python can find it
    pass


def check_size(runtime_dir):
    """Report bundle size."""
    total = 0
    for path in runtime_dir.rglob("*"):
        if path.is_file():
            total += path.stat().st_size

    mb = total / (1024 * 1024)
    print(f"\n=== Bundle Summary ===")
    print(f"  Total size: {mb:.1f} MB")
    print(f"  Location: {runtime_dir}")
    print()

    if mb > 500:
        print("  WARNING: Bundle is large. Consider:")
        print("  - Removing unused Python stdlib modules")
        print("  - Only bundling essential Hermes Agent dependencies")
        print("  - Using UPX compression for the final installer")


def main():
    args = parse_args()

    print("=" * 50)
    print("Hermes Windows Runtime Bundler")
    print("=" * 50)

    python_ver = args.python_version
    arch = args.arch

    if python_ver not in PYTHON_DOWNLOADS:
        print(f"Unsupported Python version: {python_ver}")
        print(f"Available: {list(PYTHON_DOWNLOADS.keys())}")
        sys.exit(1)

    if arch not in PYTHON_DOWNLOADS[python_ver]:
        print(f"Unsupported arch {arch} for Python {python_ver}")
        sys.exit(1)

    hermes_source = args.hermes_source or str(Path.home() / "hermes-agent")

    # Clean runtime directory
    if RUNTIME_DIR.exists():
        print(f"\nCleaning existing runtime directory: {RUNTIME_DIR}")
        shutil.rmtree(RUNTIME_DIR)

    # Download Python if needed
    python_url = PYTHON_DOWNLOADS[python_ver][arch]
    python_zip = RUNTIME_DIR / f"python-{python_ver}-{arch}.zip"

    RUNTIME_DIR.mkdir(parents=True, exist_ok=True)

    if not args.skip_download or not python_zip.exists():
        print(f"\nDownloading Python {python_ver} ({arch})...")
        download_file(python_url, python_zip)
    else:
        print(f"\nUsing cached Python zip: {python_zip}")

    # Extract and setup
    setup_embeddable_python(python_zip, PYTHON_DIR)
    install_dependencies(PYTHON_DIR)
    copy_hermes_agent(hermes_source, AGENT_DIR)

    # Remove the zip to save space
    python_zip.unlink()

    check_size(RUNTIME_DIR)

    print("\nDone! Runtime bundle is ready for Tauri build.")
    print(f"The Tauri Rust code will look for Python at: {PYTHON_DIR}")
    print(f"And Hermes Agent at: {AGENT_DIR}")


if __name__ == "__main__":
    main()
