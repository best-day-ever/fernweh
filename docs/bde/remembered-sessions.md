# Remembered Fernweh sign-ins

Device-bound remote sign-ins last up to **90 days**, including inactivity. Both
password + authenticator and passkey sign-ins request a renewable ticket with
that limit. The client already restores its protected session token after app
restarts on macOS (Keychain) and Windows (current-user DPAPI). No password or
one-time code is stored. Unbound sign-ins retain their shorter existing limits.

The broker and the directory second-factor renewable-ticket policy must both
allow 7,776,000 seconds. The directory may grant less; existing tickets and
sessions retain the expiry they were issued with. A broker restart preserves
valid device-bound sessions, but cannot revive an expired Kerberos ticket.
A prolonged broker/KDC outage can therefore require a new sign-in sooner.

Long-lived login does not preserve access after revocation. Every authenticated
operation checks current account status. Workstation admission and keepalive
also check HBAC authorization. The five-second reaper checks inactive sessions
and ends sessions with disabled/deleted accounts, no remaining grants, or a
revoked host used since startup. The existing directory cache is 60 seconds;
revocation is normally observed within 65 seconds of its visibility in LDAP.
Expired authorization is never served through a directory outage. Temporary
outages deny operations while preserving the remembered login for retry.

Ending a session destroys its ticket and persistent record and flushes its
relay flows, including established flows whose original lease has lapsed.
Direct office streams bypass the broker's relay; ending the broker session
blocks future admissions/keepalives but does not forcibly terminate an already
running direct stream or the workstation's desktop login.

Verification: the broker fake-clock suite exercises renewal through day 90,
resuming after 65 days without using the client, absolute expiry, an explicitly
shorter idle policy, broker restart, a shorter KDC grant, disabled/deleted
accounts, revoked relay/direct grants, keepalive denial, and directory outage
recovery. These checks simulate elapsed time; they are not a real 90-day soak.
