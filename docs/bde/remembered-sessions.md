# Remembered Fernweh sign-ins

Device-bound remote sign-ins have **no automatic age or inactivity expiry**.
Both application limits use zero to mean "never"; administrators can configure
finite limits later. Password + authenticator and passkey sign-ins both use
this policy. The client already restores its protected session token after app
restarts on macOS (Keychain) and Windows (current-user DPAPI). No password or
one-time code is stored. Unbound sign-ins retain their shorter existing limits.

Kerberos still requires valid credentials. With no application expiry, the
broker requests the maximum renewable interval supported by MIT's signed
32-bit `krb5_deltat` API (2,147,483,647 seconds); the directory OTP/passkey policy
must match. This is a protocol credential horizon, not an application logout
timer. The KDC may grant less. Existing tickets/sessions keep their issued
expiry until a fresh sign-in; they cannot be extended retroactively. A broker
restart preserves valid device-bound sessions, but cannot revive an expired
Kerberos ticket after prolonged broker/KDC downtime. Credential rejection,
explicit sign-out, or loss of protected local storage can still require login.

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

Verification: the broker fake-clock suite exercises a full year without client
activity followed by broker restart and resume, finite policy configuration,
renewal up to a shorter KDC limit, an explicitly shorter idle policy,
disabled/deleted accounts, revoked relay/direct grants, keepalive denial, and
directory outage recovery. These checks simulate elapsed time; they are not a
real year-long soak.
