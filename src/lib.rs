use openssl::asn1::{Asn1Time, Asn1TimeRef};
use openssl::error::ErrorStack;
use openssl::nid::Nid;
use openssl::ssl::{HandshakeError, Ssl, SslContext, SslMethod, SslVerifyMode};
use openssl::x509::{X509NameEntries, X509};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::io::Error;
use std::net::{TcpStream, ToSocketAddrs};
use std::ops::Deref;
use std::time::Duration;

static TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Serialize, Deserialize)]
pub struct Chain {
    pub subject: String,
    pub issuer: String,
    pub valid_from: String,
    pub valid_to: String,
    pub signature_algorithm: String,
}

#[derive(Serialize, Deserialize)]
pub struct Certificate {
    pub hostname: String,
    pub subject: Subject,
    pub issued: Issuer,
    pub valid_from: String,
    pub valid_to: String,
    pub validity_days: i32,
    pub is_expired: bool,
    pub cert_sn: String,
    pub cert_ver: String,
    pub cert_alg: String,
    pub sans: Vec<String>,
    pub chain: Option<Vec<Chain>>,
}

#[derive(Serialize, Deserialize)]
pub struct Issuer {
    pub country_or_region: String,
    pub organization: String,
    pub common_name: String,
}

#[derive(Serialize, Deserialize)]
pub struct Subject {
    pub country_or_region: String,
    pub state_or_province: String,
    pub locality: String,
    pub organization_unit: String,
    pub organization: String,
    pub common_name: String,
}

impl Certificate {
    pub fn from(host: &str) -> Result<Certificate, TLSValidationError> {
        let mut context = SslContext::builder(SslMethod::tls())?;
        context.set_verify(SslVerifyMode::empty());
        let context_builder = context.build();

        let mut connector = Ssl::new(&context_builder)?;
        connector.set_hostname(host)?;
        let remote = format!("{}:443", host);

        let mut address = remote.to_socket_addrs()?;

        let socket_addr = address.next().unwrap();
        let tcp_stream = TcpStream::connect_timeout(&socket_addr, TIMEOUT)?;

        tcp_stream.set_read_timeout(Some(TIMEOUT))?;
        let stream = connector.connect(tcp_stream)?;

        // `Ssl` object associated with this stream
        let ssl = stream.ssl();

        let peer_cert_chain = ssl
            .peer_cert_chain()
            .ok_or("Peer certificate chain not found")?
            .iter()
            .map(|chain| Chain {
                subject: from_entries(chain.subject_name().entries_by_nid(Nid::COMMONNAME)),
                valid_to: chain.not_after().to_string(),
                valid_from: chain.not_before().to_string(),
                issuer: from_entries(chain.issuer_name().entries_by_nid(Nid::COMMONNAME)),
                signature_algorithm: chain.signature_algorithm().object().to_string(),
            })
            .collect::<Vec<Chain>>();

        let x509_ref = ssl.peer_certificate().ok_or("Certificate not found")?;

        let data = get_certificate_info(&x509_ref);
        let certificate = Certificate {
            hostname: host.to_string(),
            subject: data.subject,
            issued: data.issued,
            valid_from: data.valid_from,
            valid_to: data.valid_to,
            validity_days: data.validity_days,
            is_expired: data.is_expired,
            cert_sn: data.cert_sn,
            cert_ver: data.cert_ver,
            cert_alg: data.cert_alg,
            sans: data.sans,
            chain: Some(peer_cert_chain),
        };
        Ok(certificate)
    }
}

fn from_entries(mut entries: X509NameEntries) -> String {
    match entries.next() {
        None => "None".to_string(),
        Some(x509_name_ref) => x509_name_ref.data().as_utf8().unwrap().to_string(),
    }
}

fn get_subject(cert_ref: &X509) -> Subject {
    let subject_country_region =
        from_entries(cert_ref.subject_name().entries_by_nid(Nid::COUNTRYNAME));
    let subject_state_province = from_entries(
        cert_ref
            .subject_name()
            .entries_by_nid(Nid::STATEORPROVINCENAME),
    );
    let subject_locality = from_entries(cert_ref.subject_name().entries_by_nid(Nid::LOCALITYNAME));
    let subject_organization_unit = from_entries(
        cert_ref
            .subject_name()
            .entries_by_nid(Nid::ORGANIZATIONALUNITNAME),
    );
    let subject_common_name = from_entries(cert_ref.subject_name().entries_by_nid(Nid::COMMONNAME));
    let organization_name = from_entries(
        cert_ref
            .subject_name()
            .entries_by_nid(Nid::ORGANIZATIONNAME),
    );

    Subject {
        country_or_region: subject_country_region,
        state_or_province: subject_state_province,
        locality: subject_locality,
        organization_unit: subject_organization_unit,
        organization: organization_name,
        common_name: subject_common_name,
    }
}

fn get_issuer(cert_ref: &X509) -> Issuer {
    let issuer_common_name = from_entries(cert_ref.issuer_name().entries_by_nid(Nid::COMMONNAME));
    let issuer_organization_name =
        from_entries(cert_ref.issuer_name().entries_by_nid(Nid::ORGANIZATIONNAME));
    let issuer_country_region =
        from_entries(cert_ref.issuer_name().entries_by_nid(Nid::COUNTRYNAME));
    Issuer {
        country_or_region: issuer_country_region,
        organization: issuer_organization_name,
        common_name: issuer_common_name,
    }
}

fn get_certificate_info(cert_ref: &X509) -> Certificate {
    let mut sans = Vec::new();
    match cert_ref.subject_alt_names() {
        None => {}
        Some(general_names) => {
            for general_name in general_names {
                sans.push(general_name.dnsname().unwrap().to_string());
            }
        }
    }
    return Certificate {
        hostname: "None".to_string(),
        subject: get_subject(cert_ref),
        issued: get_issuer(cert_ref),
        valid_from: cert_ref.not_before().to_string(),
        valid_to: cert_ref.not_after().to_string(),
        validity_days: get_validity_days(cert_ref.not_after()),
        is_expired: has_expired(cert_ref.not_after()),
        cert_sn: cert_ref.serial_number().to_bn().unwrap().to_string(),
        cert_ver: cert_ref.version().to_string(),
        cert_alg: cert_ref.signature_algorithm().object().to_string(),
        sans,
        chain: None,
    };
}

fn get_validity_days(not_after: &Asn1TimeRef) -> i32 {
    return Asn1Time::days_from_now(0)
        .unwrap()
        .deref()
        .diff(not_after)
        .unwrap()
        .days;
}

fn has_expired(not_after: &Asn1TimeRef) -> bool {
    not_after < Asn1Time::days_from_now(0).unwrap()
}

#[derive(Debug)]
pub struct TLSValidationError {
    pub details: String,
}

impl TLSValidationError {
    fn new(msg: &str) -> TLSValidationError {
        TLSValidationError {
            details: msg.to_string(),
        }
    }
}

impl From<Error> for TLSValidationError {
    fn from(e: Error) -> TLSValidationError {
        TLSValidationError::new(&e.to_string())
    }
}

impl From<&str> for TLSValidationError {
    fn from(e: &str) -> TLSValidationError {
        TLSValidationError::new(e)
    }
}

impl From<ErrorStack> for TLSValidationError {
    fn from(e: ErrorStack) -> TLSValidationError {
        TLSValidationError::new(&e.to_string())
    }
}

impl<S> From<HandshakeError<S>> for TLSValidationError {
    fn from(_: HandshakeError<S>) -> TLSValidationError {
        TLSValidationError::new("TLS handshake failed.")
    }
}

#[cfg(test)]
mod tests {

    use crate::Certificate;

    #[test]
    fn test_check_tls_for_expired_host() {
        let host = "expired.badssl.com";
        let cert = Certificate::from(host).unwrap();
        println!("Expired: {}", cert.is_expired);
        assert_eq!(cert.is_expired, true);
        assert_eq!(cert.cert_alg, "sha256WithRSAEncryption");
        assert_eq!(cert.subject.common_name, "*.badssl.com");
        assert_eq!(cert.subject.organization, "None");
        assert_eq!(
            cert.issued.common_name,
            "COMODO RSA Domain Validation Secure Server CA"
        );
        assert!(cert.validity_days < 0);
        assert_eq!(cert.cert_sn, "99565320202650452861752791156765321481");
        assert_eq!(cert.cert_ver, "2");
        assert_eq!(cert.hostname, host)
    }

    #[test]
    fn test_check_tls_for_valid_host() {
        let host = "jpbd.dev";
        let cert = Certificate::from(host).unwrap();
        println!("Expired: {}", cert.is_expired);
        assert_eq!(cert.is_expired, false);
        assert_eq!(cert.cert_alg, "ecdsa-with-SHA384");
        assert_eq!(cert.subject.common_name, host);
        assert_eq!(cert.subject.organization, "None");
        assert_eq!(cert.issued.common_name, "E1");
        assert!(cert.validity_days > 0);
        assert!(!cert.cert_sn.is_empty());
        assert_eq!(cert.cert_ver, "2");
        assert_eq!(cert.sans.len(), 2);
        assert_eq!(cert.hostname, host);
        assert!(!cert.chain.unwrap().is_empty());
    }

    #[test]
    fn test_check_tls_for_valid_host_without_sans() {
        let host = "acme-staging-v02.api.letsencrypt.org";
        let cert = Certificate::from(host).unwrap();
        assert_eq!(cert.is_expired, false);
        assert!(cert.validity_days > 0);
        assert!(!cert.sans.is_empty());

        assert_eq!(cert.subject.country_or_region, "None");
        assert_eq!(cert.subject.state_or_province, "None");
        assert_eq!(cert.subject.locality, "None");
        assert_eq!(cert.subject.organization_unit, "None");
        assert_eq!(cert.subject.organization, "None");
        assert!(!cert.subject.common_name.is_empty());

        assert_eq!(cert.issued.common_name, "R3");
        assert_eq!(cert.issued.organization, "Let's Encrypt");
        assert_eq!(cert.issued.country_or_region, "US");
        assert_eq!(cert.hostname, host);

        assert!(!cert.chain.unwrap().is_empty());
    }

    #[test]
    fn test_check_resolve_invalid_host() {
        let host = "basdomain.xyz";
        let result = Certificate::from(host).err();
        assert_eq!(
            "failed to lookup address information: nodename nor servname provided, or not known",
            result.unwrap().details
        );
    }

    #[test]
    fn test_check_tls_connection_refused() {
        let host = "slackware.com";
        let result = Certificate::from(host).err();
        let message = result.unwrap().details;
        assert!(!message.is_empty());
        println!("{}", message);
    }
}
