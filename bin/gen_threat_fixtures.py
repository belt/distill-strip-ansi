#!/usr/bin/env python3
"""Generate threat detection test fixtures.

Embeds all 6 builtin threat types in realistic protocol transcripts
that a human can audit with telnet/curl. The ESC bytes are real 0x1B,
not printable placeholders.

Threat types embedded:
  csi_21t      — CSI 21t  title report echoback (CVE-2003-0063)
  csi_6n       — CSI 6n   cursor position report
  dcs_decrqss  — DCS $q   DECRQSS echoback (CVE-2008-2383)
  osc_50       — OSC 50   font query echoback (CVE-2022-45063)
  osc_clipboard — OSC 52  clipboard access
  dcs_other    — DCS (non-query)

Output: tests/fixtures/smtp-threats.raw.txt
        tests/fixtures/http-threats.raw.txt
"""
from pathlib import Path

ESC = "\x1b"
BEL = "\x07"
ST = f"{ESC}\\"  # String Terminator

FIXTURES = Path("tests/fixtures")


def smtp_fixture() -> str:
    """SMTP session with a malicious mail server.

    Scenario: attacker controls an SMTP server. Victim runs:
      telnet mail.evil.com 25 | tee session.log
    The server injects echoback vectors into banner and responses.
    """
    lines = [
        f"220 mail.example.com ESMTP ready{ESC}[21tPAYLOAD",
        "EHLO client.local",
        "250-mail.example.com",
        "250-PIPELINING",
        "250-SIZE 10485760",
        f"250-STARTTLS{ESC}[6nPAYLOAD",
        "250-AUTH PLAIN LOGIN",
        "250 8BITMIME",
        "MAIL FROM:<build-bot@example.com>",
        "250 2.1.0 Ok",
        "RCPT TO:<admin@example.com>",
        "250 2.1.5 Ok",
        "DATA",
        "354 End data with <CR><LF>.<CR><LF>",
        "From: build-bot@example.com",
        "To: admin@example.com",
        "Subject: CI build #4821 — staging deploy",
        "Date: Mon, 07 Apr 2026 14:32:00 +0000",
        "Content-Type: text/plain; charset=utf-8",
        "",
        f"{ESC}[1mBuild Report{ESC}[0m",
        f"{ESC}[32m✓{ESC}[0m All 247 tests passed",
        f"{ESC}[33m⚠{ESC}[0m 3 warnings (unused imports)",
        "",
        "-- deployment log --",
        f"Pulling image: {ESC}P$qPAYLOAD{ST}sha256:abcdef1234567890",
        f"Config loaded: {ESC}]50;PAYLOAD{BEL}from /etc/vault/token",
        f"Deploy key: {ESC}]52;c;PAYLOAD{ST}copied to agent",
        f"DCS status: {ESC}PPAYLOAD{ST}pipeline complete",
        f"{ESC}[32m✓{ESC}[0m Deployed to staging-04.internal",
        ".",
        "250 2.0.0 Ok: queued as ABC123DEF",
        "QUIT",
        "221 2.0.0 Bye",
    ]
    return "\n".join(lines) + "\n"


def http_fixture() -> str:
    """HTTP session with a malicious web server.

    Scenario: attacker controls a web server. Victim runs:
      curl -v https://api.evil.com/status 2>&1 | strip-ansi
    The server injects echoback vectors into headers and body.
    """
    lines = [
        "* Connected to api.example.com (93.184.216.34) port 443",
        "* TLS 1.3 connection using TLS_AES_256_GCM_SHA384",
        "> GET /v1/deploy/status HTTP/1.1",
        "> Host: api.example.com",
        "> Accept: application/json",
        "> Authorization: Bearer [REDACTED]",
        ">",
        f"< HTTP/1.1 200 OK{ESC}[21tPAYLOAD",
        "< Content-Type: application/json",
        f"< X-Request-Id: req-7f3a{ESC}[6nPAYLOAD",
        "< Content-Length: 847",
        "<",
        "{",
        '  "status": "deploying",',
        '  "pipeline": [',
        f'    {{"step": "build", "log": "{ESC}[1mCompiling{ESC}[0m crate v0.4.2"}},',
        f'    {{"step": "test", "log": "{ESC}[32m✓{ESC}[0m 560 passed"}},',
        f'    {{"step": "audit", "log": "{ESC}P$qPAYLOAD{ST}checking advisories"}},',
        f'    {{"step": "font", "log": "{ESC}]50;PAYLOAD{BEL}loading assets"}},',
        f'    {{"step": "keys", "log": "{ESC}]52;c;PAYLOAD{ST}rotating"}},',
        f'    {{"step": "sync", "log": "{ESC}PPAYLOAD{ST}replicas healthy"}}',
        "  ],",
        f'  "version": "{ESC}[36mv0.4.2{ESC}[0m",',
        '  "host": "staging-04.internal"',
        "}",
        "* Connection #0 left intact",
    ]
    return "\n".join(lines) + "\n"


def expected_threats() -> str:
    """Document which threats appear and at which lines.

    This file is for human reference, not machine parsing.
    """
    return """\
# Threat map for smtp-threats.raw.txt and http-threats.raw.txt
#
# Each fixture contains all 6 builtin threat types:
#
#   csi_21t      — title report echoback (CVE-2003-0063)
#   csi_6n       — cursor position report
#   dcs_decrqss  — DECRQSS echoback (CVE-2008-2383)
#   osc_50       — font query echoback (CVE-2022-45063)
#   osc_clipboard — OSC 52 clipboard access
#   dcs_other    — non-query DCS
#
# smtp-threats.raw.txt:
#   Line 1:  csi_21t      (in 220 banner)
#   Line 6:  csi_6n       (in EHLO response)
#   Line 25: dcs_decrqss  (in message body, deploy log)
#   Line 26: osc_50       (in message body, deploy log)
#   Line 27: osc_clipboard (in message body, deploy log)
#   Line 28: dcs_other    (in message body, deploy log)
#
# http-threats.raw.txt:
#   Line 8:  csi_21t      (in HTTP status line)
#   Line 10: csi_6n       (in response header)
#   Line 17: dcs_decrqss  (in JSON body, pipeline log)
#   Line 18: osc_50       (in JSON body, pipeline log)
#   Line 19: osc_clipboard (in JSON body, pipeline log)
#   Line 20: dcs_other    (in JSON body, pipeline log)
"""


if __name__ == "__main__":
    FIXTURES.mkdir(parents=True, exist_ok=True)

    smtp = FIXTURES / "smtp-threats.raw.txt"
    smtp.write_text(smtp_fixture())
    print(f"Wrote {smtp} ({smtp.stat().st_size} bytes)")

    http = FIXTURES / "http-threats.raw.txt"
    http.write_text(http_fixture())
    print(f"Wrote {http} ({http.stat().st_size} bytes)")

    threat_map = FIXTURES / "threat-fixtures.map.txt"
    threat_map.write_text(expected_threats())
    print(f"Wrote {threat_map}")
