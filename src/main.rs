use clap::{App, Arg};
use std::process::exit;
use tlschecker::Certificate;

fn main() {
    let matches = App::new("TLS Checker")
        .version(env!("CARGO_PKG_VERSION"))
        .author(env!("CARGO_PKG_AUTHORS"))
        .long_about(env!("CARGO_PKG_DESCRIPTION"))
        .arg(
            Arg::with_name("host")
                .short("h")
                .takes_value(true)
                .multiple(true)
                .required(true)
                .help("Set hostname to check"),
        )
        .arg(
            Arg::with_name("json")
                .long("json")
                .help("Prints json output"),
        )
        .get_matches();

    let hosts = matches.values_of("host").unwrap();

    let mut certificates = Vec::with_capacity(hosts.len());

    for host in hosts {
        match Certificate::from(host) {
            Ok(cert) => {
                certificates.push(cert);
            }
            Err(err) => {
                println!("Fail to check host: {}  {} ", &host, &err.details);
            }
        }
    }

    if !matches.is_present("json") {
        for cert in certificates {
            println!("--------------------------------------");
            println!("Hostname: {}", cert.hostname);
            println!("Issued domain: {}", cert.subject.common_name);
            println!("Subject Name :");
            println!("\tCountry or Region: {}", cert.subject.country_or_region);
            println!("\tState or Province: {}", cert.subject.state_or_province);
            println!("\tLocality: {}", cert.subject.locality);
            println!("\tOrganizational Unit: {}", cert.subject.organization_unit);
            println!("\tOrganization: {}", cert.subject.organization);
            println!("\tCommon Name: {}", cert.subject.common_name);
            println!("Issuer Name:");
            println!("\tCountry or Region: {}", cert.issued.country_or_region);
            println!("\tOrganization: {}", cert.issued.organization);
            println!("\tCommon Name: {}", cert.issued.common_name);
            println!("Valid from: {}", cert.valid_from);
            println!("Valid to: {}", cert.valid_to);
            println!("Days left: {}", cert.validity_days);
            println!("Expired: {}", cert.is_expired);
            println!("Certificate version: {}", cert.cert_ver);
            println!("Certificate algorithm: {}", cert.cert_alg);
            println!("Certificate S/N: {}", cert.cert_sn);
            println!("Subject Alternative Names:");
            for san in cert.sans {
                println!("\tDNS Name: {}", san);
            }
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&certificates).unwrap());
    }

    exit(0);
}
