use crate::arango::{
    cleanup_test_database, get_database_connection_pool, get_database_connection_pool_mock,
    prepare_empty_test_database,
};
use crate::global_configuration::GlobalConfiguration;
use crate::graphql_schema::create_graphql_schema;
use juniper::http::GraphQLRequest;
use juniper::DefaultScalarValue;
use warp::http::StatusCode;
use warp::test::request;

async fn create_combined_filter_with_database(
    db_name: &str,
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    prepare_empty_test_database(db_name).await;

    let pool = get_database_connection_pool(
        // TODO: make it more DX/test friendly (?)
        // TODO: dev k8s cluster
        "http://arangodb-single-server.default.svc.cluster.local:8529",
        db_name,
        "",
        "",
    );

    crate::warp_server::filters::combined_filter(
        &pool,
        create_graphql_schema(),
        &GlobalConfiguration {
            ..Default::default()
        },
    )

    // TODO: DB cleanup (?)
}

fn create_combined_filter_without_database(
) -> impl warp::Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    crate::warp_server::filters::combined_filter(
        &get_database_connection_pool_mock(),
        create_graphql_schema(),
        &GlobalConfiguration {
            ..Default::default()
        },
    )
}

#[tokio::test]
async fn test_server_unknown_route() {
    let resp = request()
        .method("GET")
        .path("/unknown/route/yadada")
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_graphql_post_query() {
    let resp = request()
        .method("POST")
        .path("/graphql")
        .json(&GraphQLRequest::<DefaultScalarValue>::new(
            String::from("query{__typename}"),
            None,
            None,
        ))
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.body(), r#"{"data":{"__typename":"Query"}}"#);
}

#[tokio::test]
async fn test_graphql_post_mutation() {
    let resp = request()
        .method("POST")
        .path("/graphql")
        .json(&GraphQLRequest::<DefaultScalarValue>::new(
            String::from("mutation{__typename}"),
            None,
            None,
        ))
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.body(), r#"{"data":{"__typename":"Mutation"}}"#);
}

#[ignore]
#[tokio::test]
async fn test_graphql_post_forbidden() {
    let resp = request()
        .method("POST")
        .path("/graphql")
        .header("authorization", "Bearer XYZ")
        .json(&GraphQLRequest::<DefaultScalarValue>::new(
            String::from("query{__typename}"),
            None,
            None,
        ))
        .reply(&create_combined_filter_with_database("abacus_test_graphql_post_forbidden").await)
        .await;

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        resp.body(),
        r#"{"code":403,"message":"Session token doesn't match any user."}"#
    );

    cleanup_test_database("abacus_test_graphql_post_forbidden").await;
}

#[ignore]
#[tokio::test]
async fn test_graphql_post_query_fail() {
    let resp = request()
        .method("POST")
        .path("/graphql")
        .json(&GraphQLRequest::<DefaultScalarValue>::new(
            String::from("query{__wtf}"),
            None,
            None,
        ))
        .reply(&create_combined_filter_with_database("abacus_test_graphql_post_query_fail").await)
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.body(),
        r#"{"errors":[{"message":"Unknown field \"__wtf\" on type \"Query\"","locations":[{"line":1,"column":7}]}]}"#
    );

    cleanup_test_database("abacus_test_graphql_post_query_fail").await;
}

#[tokio::test]
async fn test_graphql_post_syntax_error() {
    let resp = request()
        .method("POST")
        .path("/graphql")
        .json(&GraphQLRequest::<DefaultScalarValue>::new(
            String::from("query{syntax$"),
            None,
            None,
        ))
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.body(),
        r#"{"errors":[{"message":"Unexpected \"$\"","locations":[{"line":1,"column":13}]}]}"#
    );
}

#[tokio::test]
async fn test_graphql_multipart_query() {
    let boundary = "--abcdef1234--";
    let body = format!(
        "\
         --{0}\r\n\
         content-disposition: form-data; name=\"query\"\r\n\r\n\
         query {{ __typename }}\r\n\
         --{0}--\r\n\
         ",
        boundary
    );

    let resp = request()
        .method("POST")
        .path("/graphql")
        .header("content-length", body.len())
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(body)
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.body(), r#"{"data":{"__typename":"Query"}}"#);
}

#[tokio::test]
async fn test_graphql_multipart_mutation() {
    let boundary = "--abcdef1234--";
    let body = format!(
        "\
         --{0}\r\n\
         content-disposition: form-data; name=\"query\"\r\n\r\n\
         mutation {{ __typename }}\r\n\
         --{0}--\r\n\
         ",
        boundary
    );

    let resp = request()
        .method("POST")
        .path("/graphql")
        .header("content-length", body.len())
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(body)
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.body(), r#"{"data":{"__typename":"Mutation"}}"#);
}

#[ignore]
#[tokio::test]
async fn test_graphql_multipart_forbidden() {
    let boundary = "--abcdef1234--";
    let body = format!(
        "\
         --{0}\r\n\
         content-disposition: form-data; name=\"query\"\r\n\r\n\
         query {{ __typename }}\r\n\
         --{0}--\r\n\
         ",
        boundary
    );

    let resp = request()
        .method("POST")
        .path("/graphql")
        .header("authorization", "Bearer XYZ")
        .header("content-length", body.len())
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(body)
        .reply(
            &create_combined_filter_with_database("abacus_test_graphql_multipart_forbidden").await,
        )
        .await;

    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        resp.body(),
        r#"{"code":403,"message":"Session token doesn't match any user."}"#
    );

    cleanup_test_database("abacus_test_graphql_multipart_forbidden").await;
}

#[tokio::test]
async fn test_graphql_multipart_upload_forbidden() {
    let boundary = "--abcdef1234--";
    let body = format!(
        "\
         --{0}\r\n\
         content-disposition: form-data; name=\"\"; filename=\"test.png\"\r\n\
         content-type: image/png\r\n\r\n\
         some binary content mock\r\n\
         --{0}\r\n\
         content-disposition: form-data; name=\"query\"\r\n\r\n\
         query {{ __typename }}\r\n\
         --{0}--\r\n\
         ",
        boundary
    );

    let resp = request()
        .method("POST")
        .path("/graphql")
        .header("content-length", body.len())
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(body)
        .reply(&create_combined_filter_without_database())
        .await;

    // Requests which contain additional fields (uploads) require admin auth.
    // Requests with only query/variable/operation_name are allowed without admin auth.
    assert_eq!(
        resp.body(),
        r#"{"code":403,"message":"admin permissions required when uploading"}"#
    );
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_graphql_multipart_upload_forbidden_unkown_content_type() {
    let boundary = "--abcdef1234--";
    let body = format!(
        "\
         --{0}\r\n\
         content-disposition: form-data; name=\"\"; filename=\"test.pdf\"\r\n\
         content-type: application/pdf\r\n\r\n\
         some binary content mock\r\n\
         --{0}\r\n\
         content-disposition: form-data; name=\"query\"\r\n\r\n\
         query {{ __typename }}\r\n\
         --{0}--\r\n\
         ",
        boundary
    );

    let resp = request()
        .method("POST")
        .path("/graphql")
        .header("content-length", body.len())
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(body)
        .reply(&create_combined_filter_without_database())
        .await;

    // Requests which contain additional fields (uploads) require admin auth.
    // Requests with only query/variable/operation_name are allowed without admin auth.
    assert_eq!(
        resp.body(),
        r#"{"code":400,"message":"invalid uploadable file type: application/pdf"}"#
    );
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_graphql_multipart_query_fail() {
    let boundary = "--abcdef1234--";
    let body = format!(
        "\
         --{0}\r\n\
         content-disposition: form-data; name=\"query\"\r\n\r\n\
         query {{ __wtf }}\r\n\
         --{0}--\r\n\
         ",
        boundary
    );

    let resp = request()
        .method("POST")
        .path("/graphql")
        .header("content-length", body.len())
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(body)
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.body(),
        r#"{"errors":[{"message":"Unknown field \"__wtf\" on type \"Query\"","locations":[{"line":1,"column":9}]}]}"#
    );
}

#[tokio::test]
async fn test_graphql_multipart_query_missing() {
    let boundary = "--abcdef1234--";
    let body = format!(
        "\
         --{0}\r\n\
         content-disposition: form-data; name=\"\"; filename=\"test.png\"\r\n\
         content-type: image/png\r\n\r\n\
         some binary content mock\r\n\
         --{0}--\r\n\
         ",
        boundary
    );

    let resp = request()
        .method("POST")
        .path("/graphql")
        .header("content-length", body.len())
        .header(
            "content-type",
            format!("multipart/form-data; boundary={}", boundary),
        )
        .body(body)
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        resp.body(),
        r#"{"code":400,"message":"Query is a required field when using multipart GraphQL."}"#
    );
}

#[tokio::test]
async fn test_server_unknown_method_get() {
    let resp = request()
        .method("GET")
        .path("/graphql?query={__typename}")
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn test_server_unknown_path() {
    let resp = request()
        .method("POST")
        .path("/graphqlx")
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_server_ping() {
    let resp = request()
        .method("GET")
        .path("/status/ping")
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.body(), r#"pong"#);
}

#[tokio::test]
async fn test_server_invalid_redirect() {
    let resp = request()
        .method("GET")
        // The following redirect is not a valid format (should be a valid UUID v4):
        .path("/redirect/XYZ-ABC")
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_server_webhooks_stripe_no_signature() {
    let resp = request()
        .method("POST")
        .path("/webhooks/stripe")
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        resp.body(),
        r#"Unable to verify Stripe signature because the header is missing."#
    );
}

#[tokio::test]
async fn test_server_webhooks_stripe_bad_signature() {
    let resp = request()
        .method("POST")
        .path("/webhooks/stripe")
        .header("Stripe-Signature", "__INVALID__")
        .reply(&create_combined_filter_without_database())
        .await;

    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(resp.body(), r#"Invalid signature."#);
}
