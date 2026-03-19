//! Network whitelist and SSRF protection.
//!
//! Validates URLs against a domain whitelist and blocks requests to
//! private/internal IP ranges to prevent SSRF attacks.

use std::net::IpAddr;

use clawx_types::security::SecurityDecision;

/// Network security checker with domain whitelist and SSRF protection.
#[derive(Debug, Clone)]
pub struct NetworkChecker {
    /// Allowed domain patterns (exact match or wildcard prefix like "*.example.com").
    allowed_domains: Vec<String>,
    /// Whether to block requests to private/internal IP addresses.
    block_private_ips: bool,
}

impl NetworkChecker {
    /// Create a new network checker with the given allowed domains.
    pub fn new(allowed_domains: Vec<String>, block_private_ips: bool) -> Self {
        Self {
            allowed_domains,
            block_private_ips,
        }
    }

    /// Check if a URL is allowed by the whitelist and SSRF protection.
    pub fn check_url(&self, url: &str) -> SecurityDecision {
        // Parse URL to extract host
        let host = match extract_host(url) {
            Some(h) => h,
            None => {
                return SecurityDecision::Deny {
                    reason: format!("invalid or unparseable URL: {}", url),
                }
            }
        };

        // Block private IPs (SSRF protection)
        if self.block_private_ips {
            if let Some(decision) = self.check_ssrf(&host) {
                return decision;
            }
        }

        // If no whitelist configured, allow all non-SSRF URLs
        if self.allowed_domains.is_empty() {
            return SecurityDecision::Allow;
        }

        // Check domain whitelist
        if self.is_domain_allowed(&host) {
            SecurityDecision::Allow
        } else {
            SecurityDecision::Deny {
                reason: format!("domain '{}' not in whitelist", host),
            }
        }
    }

    /// Check for SSRF — returns Some(Deny) if the host is a private IP.
    fn check_ssrf(&self, host: &str) -> Option<SecurityDecision> {
        // Check if the host is an IP address
        if let Ok(ip) = host.parse::<IpAddr>() {
            if is_private_ip(&ip) {
                return Some(SecurityDecision::Deny {
                    reason: format!("SSRF protection: {} is a private/internal IP", ip),
                });
            }
        }

        // Check well-known internal hostnames
        let lower = host.to_lowercase();
        if lower == "localhost"
            || lower == "metadata.google.internal"
            || lower.ends_with(".internal")
            || lower.ends_with(".local")
        {
            return Some(SecurityDecision::Deny {
                reason: format!("SSRF protection: {} is an internal hostname", host),
            });
        }

        None
    }

    /// Check if a domain matches the whitelist.
    fn is_domain_allowed(&self, host: &str) -> bool {
        let lower = host.to_lowercase();
        for pattern in &self.allowed_domains {
            let pat = pattern.to_lowercase();
            if let Some(base) = pat.strip_prefix("*.") {
                // Wildcard: *.example.com matches sub.example.com and example.com
                let suffix = &pat[1..]; // ".example.com"
                if lower.ends_with(suffix) || lower == base {
                    return true;
                }
            } else if lower == pat {
                return true;
            }
        }
        false
    }
}

/// Extract the host from a URL string.
fn extract_host(url: &str) -> Option<String> {
    // Handle scheme-less URLs
    let after_scheme = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        return None; // Require scheme
    };

    // Remove userinfo (user:pass@)
    let after_userinfo = if let Some(idx) = after_scheme.find('@') {
        &after_scheme[idx + 1..]
    } else {
        after_scheme
    };

    // Remove path, query, fragment
    let host_port = after_userinfo
        .split('/')
        .next()
        .unwrap_or(after_userinfo)
        .split('?')
        .next()
        .unwrap_or(after_userinfo)
        .split('#')
        .next()
        .unwrap_or(after_userinfo);

    // Remove port
    let host = if host_port.starts_with('[') {
        // IPv6: [::1]:8080
        host_port
            .find(']')
            .map(|idx| &host_port[1..idx])
            .unwrap_or(host_port)
    } else if let Some(idx) = host_port.rfind(':') {
        // Check if after ':' is all digits (port), otherwise it might be IPv6
        let after = &host_port[idx + 1..];
        if after.chars().all(|c| c.is_ascii_digit()) {
            &host_port[..idx]
        } else {
            host_port
        }
    } else {
        host_port
    };

    if host.is_empty() {
        return None;
    }

    Some(host.to_string())
}

/// Check if an IP address is in a private/internal range.
fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()          // 127.0.0.0/8
            || v4.is_private()        // 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
            || v4.is_link_local()     // 169.254.0.0/16
            || v4.is_unspecified()    // 0.0.0.0
            || v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64 // 100.64.0.0/10 (CGNAT)
            || v4.octets() == [169, 254, 169, 254] // AWS metadata
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()       // ::1
            || v6.is_unspecified() // ::
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker_with_whitelist(domains: &[&str]) -> NetworkChecker {
        NetworkChecker::new(
            domains.iter().map(|s| s.to_string()).collect(),
            true,
        )
    }

    fn open_checker() -> NetworkChecker {
        NetworkChecker::new(vec![], true)
    }

    // -------------------------------------------------------------------
    // SSRF protection tests
    // -------------------------------------------------------------------

    #[test]
    fn blocks_localhost() {
        let checker = open_checker();
        let result = checker.check_url("http://localhost:8080/api");
        assert!(matches!(result, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn blocks_loopback_ip() {
        let checker = open_checker();
        let result = checker.check_url("http://127.0.0.1/secret");
        assert!(matches!(result, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn blocks_private_10_range() {
        let checker = open_checker();
        let result = checker.check_url("http://10.0.0.1/internal");
        assert!(matches!(result, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn blocks_private_172_range() {
        let checker = open_checker();
        let result = checker.check_url("http://172.16.0.1/internal");
        assert!(matches!(result, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn blocks_private_192_range() {
        let checker = open_checker();
        let result = checker.check_url("http://192.168.1.1/admin");
        assert!(matches!(result, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn blocks_metadata_google() {
        let checker = open_checker();
        let result = checker.check_url("http://metadata.google.internal/computeMetadata/v1/");
        assert!(matches!(result, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn blocks_internal_hostnames() {
        let checker = open_checker();
        assert!(matches!(
            checker.check_url("http://service.internal/api"),
            SecurityDecision::Deny { .. }
        ));
        assert!(matches!(
            checker.check_url("http://printer.local/status"),
            SecurityDecision::Deny { .. }
        ));
    }

    #[test]
    fn blocks_ipv6_loopback() {
        let checker = open_checker();
        let result = checker.check_url("http://[::1]:8080/secret");
        assert!(matches!(result, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn blocks_zero_ip() {
        let checker = open_checker();
        let result = checker.check_url("http://0.0.0.0/");
        assert!(matches!(result, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn allows_public_ip() {
        let checker = open_checker();
        let result = checker.check_url("https://8.8.8.8/dns");
        assert_eq!(result, SecurityDecision::Allow);
    }

    #[test]
    fn allows_public_domain() {
        let checker = open_checker();
        let result = checker.check_url("https://api.example.com/v1/data");
        assert_eq!(result, SecurityDecision::Allow);
    }

    // -------------------------------------------------------------------
    // Domain whitelist tests
    // -------------------------------------------------------------------

    #[test]
    fn whitelist_exact_match() {
        let checker = checker_with_whitelist(&["api.openai.com"]);
        assert_eq!(
            checker.check_url("https://api.openai.com/v1/chat"),
            SecurityDecision::Allow,
        );
    }

    #[test]
    fn whitelist_rejects_non_listed() {
        let checker = checker_with_whitelist(&["api.openai.com"]);
        let result = checker.check_url("https://evil.com/steal");
        assert!(matches!(result, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn whitelist_wildcard_match() {
        let checker = checker_with_whitelist(&["*.anthropic.com"]);
        assert_eq!(
            checker.check_url("https://api.anthropic.com/v1/messages"),
            SecurityDecision::Allow,
        );
        assert_eq!(
            checker.check_url("https://docs.anthropic.com/guide"),
            SecurityDecision::Allow,
        );
    }

    #[test]
    fn whitelist_wildcard_matches_bare_domain() {
        let checker = checker_with_whitelist(&["*.anthropic.com"]);
        assert_eq!(
            checker.check_url("https://anthropic.com/"),
            SecurityDecision::Allow,
        );
    }

    #[test]
    fn whitelist_case_insensitive() {
        let checker = checker_with_whitelist(&["API.OpenAI.COM"]);
        assert_eq!(
            checker.check_url("https://api.openai.com/v1"),
            SecurityDecision::Allow,
        );
    }

    #[test]
    fn whitelist_multiple_domains() {
        let checker = checker_with_whitelist(&["api.openai.com", "*.anthropic.com"]);
        assert_eq!(
            checker.check_url("https://api.openai.com/v1"),
            SecurityDecision::Allow,
        );
        assert_eq!(
            checker.check_url("https://api.anthropic.com/v1"),
            SecurityDecision::Allow,
        );
        assert!(matches!(
            checker.check_url("https://evil.com"),
            SecurityDecision::Deny { .. }
        ));
    }

    // -------------------------------------------------------------------
    // URL parsing edge cases
    // -------------------------------------------------------------------

    #[test]
    fn rejects_schemeless_url() {
        let checker = open_checker();
        let result = checker.check_url("example.com/path");
        assert!(matches!(result, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn handles_url_with_port() {
        let checker = checker_with_whitelist(&["example.com"]);
        assert_eq!(
            checker.check_url("https://example.com:8443/api"),
            SecurityDecision::Allow,
        );
    }

    #[test]
    fn handles_url_with_userinfo() {
        let checker = checker_with_whitelist(&["example.com"]);
        assert_eq!(
            checker.check_url("https://user:pass@example.com/path"),
            SecurityDecision::Allow,
        );
    }

    #[test]
    fn ssrf_with_whitelist_still_blocks_private() {
        // Even if localhost is in the whitelist, SSRF protection should still block
        let checker = NetworkChecker::new(
            vec!["localhost".to_string()],
            true, // block_private_ips
        );
        let result = checker.check_url("http://localhost/admin");
        assert!(matches!(result, SecurityDecision::Deny { .. }));
    }

    #[test]
    fn no_ssrf_protection_allows_private() {
        let checker = NetworkChecker::new(vec![], false);
        let result = checker.check_url("http://127.0.0.1/admin");
        assert_eq!(result, SecurityDecision::Allow);
    }
}
