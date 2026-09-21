//! FreeIPA directory over LDAP (ldaps:// or ldap:// + StartTLS), SASL/GSSAPI bind
//! as the broker principal. The process sets `KRB5_CLIENT_KTNAME` to the broker
//! keytab and `KRB5CCNAME` to a private `MEMORY:` cache at start-up, so MIT GSSAPI
//! acquires the initiator ticket from the keytab by itself (no kinit, no cyrus-sasl).

use std::collections::{HashMap, HashSet};
use std::net::Ipv4Addr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ldap3::{ldap_escape, Ldap, LdapConnAsync, LdapConnSettings, LdapError, Scope, SearchEntry};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::CertificateDer;
use tracing::{debug, error, warn};

use crate::config::Config;
use crate::directory::{DirError, Directory};
use crate::hbac::{closure, DirectoryView, HbacRule, RemoteHost, UserView};
use crate::util::{first_rdn_value, norm_dn};

const RC_NO_SUCH_OBJECT: u32 = 32;
const RC_INSUFFICIENT_ACCESS: u32 = 50;

pub struct IpaDirectory {
    uris: Vec<String>,
    suffix: String,
    hostgroup: String,
    hbac_service: String,
    timeout: Duration,
    tls: Arc<rustls::ClientConfig>,
}

impl IpaDirectory {
    pub fn new(cfg: &Config) -> anyhow::Result<IpaDirectory> {
        let pem = std::fs::read(&cfg.ldap.ca_cert)
            .map_err(|e| anyhow::anyhow!("reading ldap.ca_cert {}: {e}", cfg.ldap.ca_cert.display()))?;
        let mut roots = rustls::RootCertStore::empty();
        for c in CertificateDer::pem_slice_iter(&pem) {
            roots.add(c?)?;
        }
        if roots.is_empty() {
            anyhow::bail!("ldap.ca_cert contains no certificate");
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])?
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(IpaDirectory {
            uris: cfg.ldap.uris.clone(),
            suffix: cfg.ldap.base_dn(&cfg.kerberos.realm),
            hostgroup: cfg.ldap.hostgroup.clone(),
            hbac_service: cfg.ldap.hbac_service.clone(),
            timeout: Duration::from_secs(cfg.ldap.timeout_secs),
            tls: Arc::new(tls),
        })
    }

    async fn connect(&self) -> Result<Ldap, DirError> {
        let mut last = String::from("no LDAP URI configured");
        for uri in &self.uris {
            match tokio::time::timeout(self.timeout, self.connect_one(uri)).await {
                Ok(Ok(l)) => return Ok(l),
                Ok(Err(e)) => last = format!("{uri}: {e}"),
                Err(_) => last = format!("{uri}: timeout"),
            }
            warn!("LDAP {last}");
        }
        Err(DirError::Unavailable(last))
    }

    async fn connect_one(&self, uri: &str) -> Result<Ldap, String> {
        let host = uri
            .split("://")
            .nth(1)
            .and_then(|r| r.split(['/', ':']).next())
            .filter(|h| !h.is_empty())
            .ok_or("bad URI")?
            .to_string();
        let settings = LdapConnSettings::new()
            .set_conn_timeout(self.timeout)
            .set_config(self.tls.clone())
            .set_starttls(uri.starts_with("ldap://"));
        let (conn, mut ldap) = LdapConnAsync::with_settings(settings, uri).await.map_err(|e| e.to_string())?;
        ldap3::drive!(conn);
        ldap.with_timeout(self.timeout);
        let res = ldap.sasl_gssapi_bind(&host).await.map_err(|e| format!("GSSAPI bind: {e}"))?;
        res.success().map_err(|e| format!("GSSAPI bind: {e}"))?;
        Ok(ldap)
    }

    async fn search(
        &self,
        ldap: &mut Ldap,
        base: &str,
        scope: Scope,
        filter: &str,
        attrs: &[&str],
    ) -> Result<Vec<SearchEntry>, DirError> {
        ldap.with_timeout(self.timeout);
        let r =
            ldap.search(base, scope, filter, attrs.to_vec()).await.map_err(|e| DirError::Unavailable(e.to_string()))?;
        match r.success() {
            Ok((entries, _)) => Ok(entries.into_iter().map(SearchEntry::construct).collect()),
            Err(LdapError::LdapResult { result }) if result.rc == RC_NO_SUCH_OBJECT => Ok(Vec::new()),
            Err(LdapError::LdapResult { result }) if result.rc == RC_INSUFFICIENT_ACCESS => {
                Err(DirError::InsufficientAccess(format!("search {base}")))
            }
            Err(e) => Err(DirError::Unavailable(format!("search {base}: {e}"))),
        }
    }

    async fn dns_a(&self, ldap: &mut Ldap, fqdn: &str) -> Result<Option<Ipv4Addr>, DirError> {
        let labels: Vec<&str> = fqdn.split('.').collect();
        for i in 1..labels.len() {
            let name = labels[..i].join(".");
            let zone = labels[i..].join(".");
            for z in [format!("{zone}."), zone.clone()] {
                let base = format!("idnsname={name},idnsname={z},cn=dns,{}", self.suffix);
                let e = self.search(ldap, &base, Scope::Base, "(objectClass=*)", &["aRecord"]).await?;
                if let Some(e) = e.first() {
                    return Ok(attr(e, "arecord").iter().find_map(|a| a.parse().ok()));
                }
            }
        }
        // Nothing found: distinguish "no record" from "cannot read cn=dns at all".
        let zones = self
            .search(ldap, &format!("cn=dns,{}", self.suffix), Scope::OneLevel, "(objectClass=idnszone)", &["idnsname"])
            .await?;
        if zones.is_empty() {
            return Err(DirError::InsufficientAccess(
                "cannot read any DNS zone under cn=dns; grant the broker principal read access to DNS records".into(),
            ));
        }
        Ok(None)
    }

    async fn load_view(&self, ldap: &mut Ldap) -> Result<DirectoryView, DirError> {
        let s = &self.suffix;
        let service_dn = format!("cn={},cn=hbacservices,cn=hbac,{s}", self.hbac_service);
        let rules_raw = self
            .search(
                ldap,
                &format!("cn=hbac,{s}"),
                Scope::OneLevel,
                "(objectClass=ipahbacrule)",
                &[
                    "cn",
                    "ipaEnabledFlag",
                    "accessRuleType",
                    "userCategory",
                    "hostCategory",
                    "serviceCategory",
                    "memberUser",
                    "memberHost",
                    "memberService",
                ],
            )
            .await?;
        let rules: Vec<HbacRule> = rules_raw
            .iter()
            .map(|e| HbacRule {
                name: attr(e, "cn").first().cloned().unwrap_or_default(),
                enabled: attr(e, "ipaenabledflag").first().map(|v| v.eq_ignore_ascii_case("TRUE")).unwrap_or(false),
                allow: attr(e, "accessruletype").first().map(|v| v.eq_ignore_ascii_case("allow")).unwrap_or(true),
                user_category_all: attr(e, "usercategory").iter().any(|v| v.eq_ignore_ascii_case("all")),
                host_category_all: attr(e, "hostcategory").iter().any(|v| v.eq_ignore_ascii_case("all")),
                service_category_all: attr(e, "servicecategory").iter().any(|v| v.eq_ignore_ascii_case("all")),
                member_users: attr(e, "memberuser"),
                member_hosts: attr(e, "memberhost"),
                member_services: attr(e, "memberservice"),
            })
            .collect();

        let svc_groups = self
            .search(
                ldap,
                &format!("cn=hbacservicegroups,cn=hbac,{s}"),
                Scope::OneLevel,
                &format!("(member={})", ldap_escape(&service_dn)),
                &["cn"],
            )
            .await?;
        let service_groups: HashSet<String> = svc_groups.iter().map(|e| norm_dn(&e.dn)).collect();

        // Host groups: parent map for hosts and nested host groups.
        let hgs = self
            .search(
                ldap,
                &format!("cn=hostgroups,cn=accounts,{s}"),
                Scope::OneLevel,
                "(objectClass=ipahostgroup)",
                &["member"],
            )
            .await?;
        let mut parents: HashMap<String, Vec<String>> = HashMap::new();
        let mut hosts: HashSet<String> = HashSet::new();
        for g in &hgs {
            let gdn = norm_dn(&g.dn);
            for m in attr(g, "member") {
                let m = norm_dn(&m);
                if m.starts_with("fqdn=") {
                    hosts.insert(m.clone());
                }
                parents.entry(m).or_default().push(gdn.clone());
            }
        }
        let remote_dn = norm_dn(&format!("cn={},cn=hostgroups,cn=accounts,{s}", self.hostgroup));
        let mut remote_hosts = Vec::new();
        let mut dns_denied = None;
        for h in hosts {
            let groups = closure(parents.get(&h).cloned().unwrap_or_default(), &parents);
            if !groups.contains(&remote_dn) {
                continue;
            }
            let Some(fqdn) = first_rdn_value(&h, "fqdn") else { continue };
            let ipv4 = match self.dns_a(ldap, &fqdn).await {
                Ok(a) => a,
                Err(DirError::InsufficientAccess(m)) => {
                    dns_denied = Some(m);
                    None
                }
                Err(e) => return Err(e),
            };
            if ipv4.is_none() {
                debug!(host = %fqdn, "no A record");
            }
            remote_hosts.push(RemoteHost { fqdn, dn: h, ipv4, hostgroups: groups });
        }
        if let Some(m) = dns_denied {
            error!("LDAP insufficient access: {m}");
        }
        remote_hosts.sort_by(|a, b| a.fqdn.cmp(&b.fqdn));

        // User groups: parent map for nested groups.
        let groups = self
            .search(
                ldap,
                &format!("cn=groups,cn=accounts,{s}"),
                Scope::OneLevel,
                "(objectClass=ipausergroup)",
                &["member"],
            )
            .await?;
        let mut group_parents: HashMap<String, Vec<String>> = HashMap::new();
        for g in &groups {
            let gdn = norm_dn(&g.dn);
            for m in attr(g, "member") {
                let m = norm_dn(&m);
                if m.contains(",cn=groups,cn=accounts,") {
                    group_parents.entry(m).or_default().push(gdn.clone());
                }
            }
        }
        Ok(DirectoryView { rules, service_dn: norm_dn(&service_dn), service_groups, remote_hosts, group_parents })
    }
}

/// Attribute values by lower-cased name.
fn attr(e: &SearchEntry, name: &str) -> Vec<String> {
    e.attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone()).unwrap_or_default()
}

#[async_trait]
impl Directory for IpaDirectory {
    async fn view(&self) -> Result<DirectoryView, DirError> {
        let mut ldap = self.connect().await?;
        let r = tokio::time::timeout(self.timeout * 6, self.load_view(&mut ldap))
            .await
            .unwrap_or_else(|_| Err(DirError::Unavailable("directory snapshot timed out".into())));
        let _ = ldap.unbind().await;
        r
    }

    async fn user(&self, uid: &str) -> Result<Option<UserView>, DirError> {
        let mut ldap = self.connect().await?;
        let base = format!("uid={},cn=users,cn=accounts,{}", ldap_escape(uid), self.suffix);
        let r = self.search(&mut ldap, &base, Scope::Base, "(objectClass=*)", &["memberOf"]).await;
        let _ = ldap.unbind().await;
        Ok(r?.into_iter().next().map(|e| UserView { groups: attr(&e, "memberof"), dn: e.dn }))
    }
}
