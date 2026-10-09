//! `network:` as NetworkManager connections in the new system, and the same list
//! getting the ISO online (iwd for wifi, systemd-networkd for static ethernet, pppd
//! for PPPoE).

use std::{fs, os::unix, path::Path, process::Command, thread, time::Duration};

use anyhow::{bail, Result};
use shell_iface::{logger::Logger, Shell};

use crate::{
    config::{Config, Connection, ConnectionType, DnsOverTls, EapMethod, IpConfig, IpMethod, Phase2, WifiSecurity},
    source::Source,
    target::{self, Target},
};

/// Proxy for everything the install downloads.
pub fn apply_proxy(cfg: &Config) {
    let Some(proxy) = &cfg.network.proxy else { return };
    for (keys, value) in [(["http_proxy", "HTTP_PROXY"], &proxy.http), (["https_proxy", "HTTPS_PROXY"], &proxy.https), (["no_proxy", "NO_PROXY"], &proxy.no_proxy)] {
        if let Some(value) = value {
            keys.iter().for_each(|k| std::env::set_var(k, value));
        }
    }
}

pub fn online() -> bool {
    Command::new("curl")
        .args(["-fsS", "--max-time", "8", "-o", "/dev/null", "https://archlinux.org"])
        .status()
        .is_ok_and(|s| s.success())
}

fn wait_online(seconds: u64) -> bool {
    (0..seconds / 2).any(|_| {
        thread::sleep(Duration::from_secs(2));
        online()
    })
}

/// Gets the ISO online unless it already is, trying connections by priority.
pub fn bring_up_iso(cfg: &Config, source: &Source, logger: &Logger) -> Result<()> {
    if online() {
        return Ok(());
    }
    let mut shell = Shell::new("Network", logger);
    let mut connections: Vec<&Connection> = cfg.network.connections.iter().filter(|c| c.autoconnect).collect();
    connections.sort_by_key(|c| -c.priority);
    for c in connections {
        println!("Connecting with {}", c.name);
        let attempt = match c.kind {
            ConnectionType::Wifi => iso_wifi(c, source, &mut shell),
            ConnectionType::Ethernet => iso_ethernet(c, &mut shell),
            ConnectionType::Pppoe => iso_pppoe(c, &mut shell),
        };
        match attempt {
            Ok(()) if wait_online(30) => return Ok(()),
            Ok(()) => println!("{} connected but can't reach archlinux.org", c.name),
            Err(e) => println!("{} failed: {e:#}", c.name),
        }
    }
    // ethernet with DHCP needs nothing from us, it may just be slow
    if wait_online(20) {
        return Ok(());
    }
    bail!("not online: plug in ethernet, or check network.connections")
}

fn first_wireless() -> Option<String> {
    fs::read_dir("/sys/class/net").ok()?.flatten().find(|e| e.path().join("wireless").exists()).map(|e| e.file_name().to_string_lossy().to_string())
}

/// iwd's file name for an SSID
fn iwd_name(ssid: &str) -> String {
    if ssid.chars().all(|c| c.is_ascii_alphanumeric() || " _-".contains(c)) {
        ssid.to_string()
    } else {
        format!("={}", ssid.bytes().map(|b| format!("{b:02x}")).collect::<String>())
    }
}

fn iso_wifi(c: &Connection, source: &Source, shell: &mut Shell) -> Result<()> {
    let device = c.interface.clone().or_else(first_wireless).ok_or_else(|| anyhow::anyhow!("no wifi device"))?;
    let ssid = c.ssid.as_deref().unwrap_or_default();
    let connect = if c.hidden { "connect-hidden" } else { "connect" };
    let quoted = shell_words::quote(ssid);
    match c.security() {
        WifiSecurity::WpaPsk | WifiSecurity::Sae => {
            let password = shell_words::quote(c.password.as_deref().unwrap_or_default()).to_string();
            shell.run_and_wait_with_args("iwctl", &format!("--passphrase {password} station {device} {connect} {quoted}"))?;
        }
        WifiSecurity::Open | WifiSecurity::Owe => {
            shell.run_and_wait_with_args("iwctl", &format!("station {device} {connect} {quoted}"))?;
        }
        WifiSecurity::Eap => {
            let certs = Path::new("/var/lib/iwd/certs");
            fs::create_dir_all(certs)?;
            let cert = |name: &Option<String>| -> Result<Option<String>> {
                let Some(name) = name else { return Ok(None) };
                let local = source.fetch(name)?;
                let dest = certs.join(local.file_name().unwrap_or_default());
                fs::copy(&local, &dest)?;
                Ok(Some(dest.display().to_string()))
            };
            let eap = c.eap.as_ref().ok_or_else(|| anyhow::anyhow!("security: eap without eap settings"))?;
            let ca = cert(&eap.ca_cert)?;
            let client = cert(&eap.client_cert)?;
            let key = cert(&eap.private_key)?;
            let mut file = String::from("[Security]\n");
            let (method, prefix) = match eap.method {
                EapMethod::Peap => ("PEAP", "EAP-PEAP"),
                EapMethod::Ttls => ("TTLS", "EAP-TTLS"),
                EapMethod::Tls => ("TLS", "EAP-TLS"),
            };
            file += &format!("EAP-Method={method}\nEAP-Identity={}\n", eap.anonymous_identity.as_ref().unwrap_or(&eap.identity));
            if let Some(ca) = ca {
                file += &format!("{prefix}-CACert={ca}\n");
            }
            if let Some(domain) = &eap.domain {
                file += &format!("{prefix}-ServerDomainMask={domain}\n");
            }
            if eap.method == EapMethod::Tls {
                file += &format!("EAP-TLS-ClientCert={}\nEAP-TLS-ClientKey={}\n", client.unwrap_or_default(), key.unwrap_or_default());
                if let Some(pass) = &eap.private_key_password {
                    file += &format!("EAP-TLS-ClientKeyPassphrase={pass}\n");
                }
            } else {
                let phase2 = match (eap.method, eap.phase2) {
                    (EapMethod::Peap, Phase2::Gtc) => "GTC",
                    (EapMethod::Peap, _) => "MSCHAPV2",
                    (_, Phase2::Mschapv2) => "Tunneled-MSCHAPv2",
                    (_, Phase2::Mschap) => "Tunneled-MSCHAP",
                    (_, Phase2::Pap) => "Tunneled-PAP",
                    (_, Phase2::Chap) => "Tunneled-CHAP",
                    (_, Phase2::Gtc) => "Tunneled-GTC",
                };
                file += &format!(
                    "{prefix}-Phase2-Method={phase2}\n{prefix}-Phase2-Identity={}\n{prefix}-Phase2-Password={}\n",
                    eap.identity,
                    eap.password.as_deref().unwrap_or_default()
                );
            }
            file += &format!("\n[Settings]\nAutoConnect=true\nHidden={}\n", c.hidden);
            fs::write(format!("/var/lib/iwd/{}.8021x", iwd_name(ssid)), file)?;
            thread::sleep(Duration::from_secs(1));
            shell.run_and_wait_with_args("iwctl", &format!("station {device} {connect} {quoted}"))?;
        }
    }
    Ok(())
}

/// DHCP is the ISO's default; only a static setup needs a networkd file.
fn iso_ethernet(c: &Connection, shell: &mut Shell) -> Result<()> {
    let ip = c.ipv4.settings();
    if ip.method != IpMethod::Manual {
        return Ok(());
    }
    let name = c.interface.clone().unwrap_or("en* eth*".into());
    let mut file = format!("[Match]\nName={name}\n\n[Network]\nAddress={}\n", ip.address.unwrap_or_default());
    if let Some(gateway) = ip.gateway {
        file += &format!("Gateway={gateway}\n");
    }
    for dns in ip.dns.unwrap_or_default() {
        file += &format!("DNS={dns}\n");
    }
    fs::create_dir_all("/etc/systemd/network")?;
    fs::write("/etc/systemd/network/05-2lazy4arch.network", file)?;
    shell.run_and_wait_with_args("networkctl", "reload")?;
    Ok(())
}

fn iso_pppoe(c: &Connection, shell: &mut Shell) -> Result<()> {
    let base = c.interface.clone().unwrap_or_default();
    shell.run_and_wait_with_args("ip", &format!("link set {base} up"))?;
    let device = match c.vlan {
        Some(vlan) => {
            let vlan_dev = format!("{base}.{vlan}");
            shell.run_and_wait_with_args("ip", &format!("link add link {base} name {vlan_dev} type vlan id {vlan}"))?;
            shell.run_and_wait_with_args("ip", &format!("link set {vlan_dev} up"))?;
            vlan_dev
        }
        None => base,
    };
    let mtu = c.mtu.unwrap_or(1492);
    let peer = format!(
        "plugin pppoe.so\nnic-{device}\nuser \"{}\"\npassword \"{}\"\nnoipdefault\ndefaultroute\nusepeerdns\npersist\nhide-password\nnoauth\nmtu {mtu}\nmru {mtu}\n",
        c.username.as_deref().unwrap_or_default(),
        c.password.as_deref().unwrap_or_default()
    );
    fs::create_dir_all("/etc/ppp/peers")?;
    fs::write("/etc/ppp/peers/2lazy4arch", peer)?;
    shell.run_and_wait_with_args("pppd", "call 2lazy4arch")?;
    // pppd hands its DNS to /etc/ppp/resolv.conf; the ISO resolves through systemd-resolved
    for _ in 0..15 {
        thread::sleep(Duration::from_secs(2));
        if let Ok(resolv) = fs::read_to_string("/etc/ppp/resolv.conf") {
            let servers: Vec<&str> = resolv.lines().filter_map(|l| l.strip_prefix("nameserver ")).collect();
            let _ = shell.run_and_wait_with_args("resolvectl", &format!("dns ppp0 {}", servers.join(" ")));
            break;
        }
    }
    Ok(())
}

/// A NetworkManager keyfile. `cert` maps a certificate named in the config to
/// where it was copied in the new system.
pub fn keyfile(c: &Connection, cert: &dyn Fn(&Option<String>) -> Option<String>) -> String {
    let kind = match c.kind {
        ConnectionType::Ethernet => "ethernet",
        ConnectionType::Wifi => "wifi",
        ConnectionType::Pppoe => "pppoe",
    };
    let mut s = format!("[connection]\nid={}\ntype={kind}\n", c.name);
    if let Some(interface) = c.interface.as_ref().filter(|_| c.kind != ConnectionType::Pppoe) {
        s += &format!("interface-name={interface}\n");
    }
    if !c.autoconnect {
        s += "autoconnect=false\n";
    }
    if c.priority != 0 {
        s += &format!("autoconnect-priority={}\n", c.priority);
    }
    let link = |section: &str| {
        let mut s = format!("\n[{section}]\n");
        if let Some(mac) = &c.mac {
            s += &format!("cloned-mac-address={mac}\n");
        }
        if let Some(mtu) = c.mtu {
            s += &format!("mtu={mtu}\n");
        }
        s
    };
    match c.kind {
        ConnectionType::Ethernet => s += &link("ethernet"),
        ConnectionType::Wifi => {
            s += &link("wifi");
            s += &format!("mode=infrastructure\nssid={}\n", c.ssid.as_deref().unwrap_or_default());
            if c.hidden {
                s += "hidden=true\n";
            }
            let password = c.password.as_deref().unwrap_or_default();
            match c.security() {
                WifiSecurity::WpaPsk => s += &format!("\n[wifi-security]\nkey-mgmt=wpa-psk\npsk={password}\n"),
                WifiSecurity::Sae => s += &format!("\n[wifi-security]\nkey-mgmt=sae\npsk={password}\n"),
                WifiSecurity::Owe => s += "\n[wifi-security]\nkey-mgmt=owe\n",
                WifiSecurity::Open => {}
                WifiSecurity::Eap => {
                    s += "\n[wifi-security]\nkey-mgmt=wpa-eap\n";
                    if let Some(eap) = &c.eap {
                        let method = match eap.method {
                            EapMethod::Peap => "peap",
                            EapMethod::Ttls => "ttls",
                            EapMethod::Tls => "tls",
                        };
                        s += &format!("\n[802-1x]\neap={method};\nidentity={}\n", eap.identity);
                        let optional = [
                            ("anonymous-identity", eap.anonymous_identity.clone()),
                            ("password", eap.password.clone()),
                            ("ca-cert", cert(&eap.ca_cert)),
                            ("domain-suffix-match", eap.domain.clone()),
                            ("client-cert", cert(&eap.client_cert)),
                            ("private-key", cert(&eap.private_key)),
                            ("private-key-password", eap.private_key_password.clone()),
                        ];
                        for (key, value) in optional {
                            if let Some(value) = value {
                                s += &format!("{key}={value}\n");
                            }
                        }
                        if eap.method != EapMethod::Tls {
                            s += &format!("phase2-auth={}\n", serde_json::to_value(eap.phase2).unwrap().as_str().unwrap_or("mschapv2"));
                        }
                    }
                }
            }
        }
        ConnectionType::Pppoe => {
            let base = c.interface.clone().unwrap_or_default();
            let parent = c.vlan.map_or(base.clone(), |v| format!("{base}.{v}"));
            s += &format!(
                "\n[pppoe]\nparent={parent}\nusername={}\npassword={}\n\n[ppp]\nmtu={}\n",
                c.username.as_deref().unwrap_or_default(),
                c.password.as_deref().unwrap_or_default(),
                c.mtu.unwrap_or(1492)
            );
        }
    }
    for (section, ip) in [("ipv4", &c.ipv4), ("ipv6", &c.ipv6)] {
        s += &ip_section(section, ip);
    }
    s
}

fn ip_section(section: &str, ip: &IpConfig) -> String {
    let ip = ip.settings();
    let method = match ip.method {
        IpMethod::Auto => "auto",
        IpMethod::Manual => "manual",
        IpMethod::Disabled => "disabled",
    };
    let mut s = format!("\n[{section}]\nmethod={method}\n");
    if let Some(address) = &ip.address {
        s += &format!("address1={address}\n");
    }
    if let Some(gateway) = &ip.gateway {
        s += &format!("gateway={gateway}\n");
    }
    if let Some(dns) = &ip.dns {
        s += &format!("dns={};\nignore-auto-dns=true\n", dns.join(";"));
    }
    s
}

/// PPPoE over a VLAN needs the VLAN as its own connection.
fn vlan_keyfile(c: &Connection) -> Option<String> {
    let vlan = c.vlan?;
    let base = c.interface.as_deref().unwrap_or_default();
    Some(format!(
        "[connection]\nid={}-vlan\ntype=vlan\ninterface-name={base}.{vlan}\n\n[vlan]\nparent={base}\nid={vlan}\n\n[ipv4]\nmethod=disabled\n\n[ipv6]\nmethod=disabled\n",
        c.name
    ))
}

fn file_name(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() || "-_.".contains(c) { c } else { '_' }).collect()
}

/// Connections, DNS and proxy for the new system. Returns units to enable.
pub fn configure_target(cfg: &Config, source: &Source, target: &mut Target) -> Result<Vec<String>> {
    let mut units = vec![];
    for c in &cfg.network.connections {
        let mut copied: Vec<(String, String)> = vec![];
        if let Some(eap) = &c.eap {
            for name in [&eap.ca_cert, &eap.client_cert, &eap.private_key].into_iter().flatten() {
                let base = Path::new(name).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
                let dest = format!("/etc/2lazy4arch/certs/{}-{}", file_name(&c.name), file_name(&base));
                target::copy(&source.fetch(name)?, &dest, 0o600)?;
                copied.push((name.clone(), dest));
            }
        }
        let cert = |name: &Option<String>| name.as_ref().and_then(|n| copied.iter().find(|(k, _)| k == n).map(|(_, d)| d.clone()));
        let file = keyfile(c, &cert);
        let dir = "/etc/NetworkManager/system-connections";
        target::write_mode(&format!("{dir}/{}.nmconnection", file_name(&c.name)), &file, 0o600)?;
        if let Some(vlan) = vlan_keyfile(c) {
            target::write_mode(&format!("{dir}/{}-vlan.nmconnection", file_name(&c.name)), &vlan, 0o600)?;
        }
    }

    if let Some(dns) = &cfg.network.dns {
        if dns.over_tls != DnsOverTls::Off {
            let tls = if dns.over_tls == DnsOverTls::Strict { "yes" } else { "opportunistic" };
            let mut conf = format!("[Resolve]\nDNSOverTLS={tls}\n");
            if !dns.servers.is_empty() {
                conf += &format!("DNS={}\n", dns.servers.join(" "));
            }
            if !dns.search.is_empty() {
                conf += &format!("Domains={}\n", dns.search.join(" "));
            }
            target::write("/etc/systemd/resolved.conf.d/2lazy4arch.conf", &conf)?;
            target::write("/etc/NetworkManager/conf.d/2lazy4arch-dns.conf", "[main]\ndns=systemd-resolved\n")?;
            let resolv = target::path("/etc/resolv.conf");
            let _ = fs::remove_file(&resolv);
            unix::fs::symlink("/run/systemd/resolve/stub-resolv.conf", resolv)?;
            units.push("systemd-resolved".to_string());
        } else if !dns.servers.is_empty() {
            let mut conf = String::from("[global-dns]\n");
            if !dns.search.is_empty() {
                conf += &format!("searches={}\n", dns.search.join(","));
            }
            conf += &format!("\n[global-dns-domain-*]\nservers={}\n", dns.servers.join(","));
            target::write("/etc/NetworkManager/conf.d/2lazy4arch-dns.conf", &conf)?;
        }
    }

    if let Some(proxy) = &cfg.network.proxy {
        let mut env = String::new();
        for (keys, value) in [(["http_proxy", "HTTP_PROXY"], &proxy.http), (["https_proxy", "HTTPS_PROXY"], &proxy.https), (["no_proxy", "NO_PROXY"], &proxy.no_proxy)] {
            if let Some(value) = value {
                keys.iter().for_each(|k| env += &format!("{k}={value}\n"));
            }
        }
        target::append("/etc/environment", &env)?;
    }
    target.log("Network configured");
    Ok(units)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(yaml: &str) -> Connection {
        serde_norway::from_str(yaml).unwrap()
    }

    #[test]
    fn keyfiles() {
        let wifi = parse("{ name: home, type: wifi, ssid: Home-5G, password: secret, priority: 10, hidden: true }");
        assert_eq!(
            keyfile(&wifi, &|_| None),
            "[connection]\nid=home\ntype=wifi\nautoconnect-priority=10\n\n[wifi]\nmode=infrastructure\nssid=Home-5G\nhidden=true\n\n[wifi-security]\nkey-mgmt=wpa-psk\npsk=secret\n\n[ipv4]\nmethod=auto\n\n[ipv6]\nmethod=auto\n"
        );

        let wired = parse("{ name: wired, type: ethernet, interface: enp3s0, ipv4: { method: manual, address: 192.168.1.50/24, gateway: 192.168.1.1, dns: [192.168.1.1] }, ipv6: disabled }");
        let file = keyfile(&wired, &|_| None);
        assert!(file.contains("interface-name=enp3s0\n"), "{file}");
        assert!(file.contains("[ipv4]\nmethod=manual\naddress1=192.168.1.50/24\ngateway=192.168.1.1\ndns=192.168.1.1;\nignore-auto-dns=true\n"), "{file}");
        assert!(file.ends_with("[ipv6]\nmethod=disabled\n"), "{file}");

        let eap = parse("{ name: office, type: wifi, ssid: CorpNet, security: eap, eap: { method: peap, identity: me, password: pw, ca_cert: ./ca.pem, domain: radius.corp } }");
        let file = keyfile(&eap, &|c| c.as_ref().map(|_| "/etc/2lazy4arch/certs/office-ca.pem".to_string()));
        assert!(file.contains("key-mgmt=wpa-eap\n\n[802-1x]\neap=peap;\nidentity=me\npassword=pw\nca-cert=/etc/2lazy4arch/certs/office-ca.pem\ndomain-suffix-match=radius.corp\nphase2-auth=mschapv2\n"), "{file}");

        let pppoe = parse("{ name: isp, type: pppoe, interface: enp3s0, vlan: 100, username: u, password: p }");
        assert!(keyfile(&pppoe, &|_| None).contains("[pppoe]\nparent=enp3s0.100\nusername=u\npassword=p\n\n[ppp]\nmtu=1492\n"));
        assert!(vlan_keyfile(&pppoe).unwrap().contains("[vlan]\nparent=enp3s0\nid=100\n"));
    }

    #[test]
    fn iwd_names() {
        assert_eq!(iwd_name("Home-5G"), "Home-5G");
        assert_eq!(iwd_name("Café"), "=436166c3a9");
    }
}
