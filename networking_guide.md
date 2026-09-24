# Maze Wars - Quick Multi-Device Setup & Audit Cheat Sheet

## 1. Quick Setup (Host - Device 1)
1. **Find Host IP:** Open PowerShell and run `ipconfig` -> note **IPv4 Address** (e.g., `192.168.1.9`).
2. **Allow UDP Port 8080 in Firewall:** Open **Admin PowerShell** and run:
   ```powershell
   New-NetFirewallRule -DisplayName "MazeWars UDP 8080" -Direction Inbound -Action Allow -Protocol UDP -LocalPort 8080 -Profile Any
   ```
3. **Start Server (WSL):**
   ```bash
   cargo run --bin server
   ```

---

## 2. Quick Setup (Client - Device 2)
1. **Build Windows Binary on Host (WSL):**
   ```bash
   cargo build --bin client --target x86_64-pc-windows-gnu --release
   ```
2. **Transfer File:** Copy `target/x86_64-pc-windows-gnu/release/client.exe` from Device 1 to Device 2.
3. **Run on Device 2:** Double-click `client.exe` or run `.\client.exe` in PowerShell.

---

## 3. Demo / Connection Steps
1. **Device 1 Client:** Launch via WSL `cargo run --bin client`, set IP to `127.0.0.1:8080`.
2. **Device 2 Client:** Launch `client.exe`, set IP to `<DEVICE_1_IP>:8080` (e.g., `192.168.1.9:8080`).
3. **Verify:** Both screens transition to **`LOBBY (2 / 2 Players connected)`** and auto-start after countdown.

---

## 4. Troubleshooting Checklist

| Symptom | Cause | Instant Fix |
| :--- | :--- | :--- |
| **Stuck on "Connecting to server..."** | Windows Firewall blocking port 8080 on Host | In Admin PowerShell on Device 1, run:<br>`Set-NetFirewallProfile -Profile Domain,Public,Private -Enabled False` |
| **Bypasses lobby into local map** | Running old client `.exe` | Re-build with `--target x86_64-pc-windows-gnu` and re-send `client.exe` to Device 2. |
| **Device 2 cannot reach host IP** | WSL networking isolated | Ensure `.wslconfig` contains `networkingMode=mirrored`, then run `wsl --shutdown` in PowerShell. |