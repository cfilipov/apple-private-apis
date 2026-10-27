use super::{check_response_status, parse_response_body};
use crate::Error;
use reqwest::StatusCode;

#[test]
fn http_failures_keep_the_operation_and_status() {
    for status in [StatusCode::UNAUTHORIZED, StatusCode::TOO_MANY_REQUESTS, StatusCode::SERVICE_UNAVAILABLE] {
        assert!(matches!(check_response_status(status, "init"),
            Err(Error::AuthenticationHttp { operation: "init", status: code }) if code == status.as_u16()));
    }
    assert!(check_response_status(StatusCode::OK, "complete").is_ok());
}

#[test]
fn malformed_responses_never_expose_body_contents() {
    for body in [
        b"<html>private-account-data</html>".as_slice(),
        b"not a plist: private-account-data".as_slice(),
        b"<plist><dict><key>Response</key><string>private-account-data</string></dict></plist>".as_slice(),
        b"<plist><dict/></plist>".as_slice(),
    ] {
        let error = parse_response_body(body, "complete").unwrap_err();
        assert!(matches!(error, Error::AuthenticationResponse { operation: "complete" }));
        assert!(!format!("{error:?}").contains("private-account-data"));
        assert!(!error.to_string().contains("private-account-data"));
    }
}

#[test]
fn valid_responses_preserve_apple_status_for_the_authentication_flow() {
    let body = b"<plist><dict><key>Response</key><dict><key>Status</key><dict><key>ec</key><integer>-20101</integer></dict></dict></dict></plist>";
    let response = parse_response_body(body, "init").unwrap();
    assert_eq!(response["Status"].as_dictionary().unwrap()["ec"].as_signed_integer(), Some(-20101));
}
