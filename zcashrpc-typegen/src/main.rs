//! In order to leverage all of Rust's type safety, this crate produces
//! a set of concrete Rust types for responses from the zcashd-RPC interface.

mod error;
mod special_cases;
use error::TypegenResult;

/// Process quizface-formatted response specifications from files, producing
/// Rust types, in the `rpc_response_types.rs` file.
fn main() {
    let initial_comment = r#"//procedurally generated response types, note that zcashrpc-typegen
           //is in early alpha, and output is subject to change at any time.
"#;
    use std::io::Write as _;
    std::fs::write(output_path(), initial_comment).unwrap();
    for filenode in std::fs::read_dir(&std::path::Path::new(
        &std::env::args()
            .nth(1)
            .unwrap_or("./example_dir".to_string()),
    ))
    .unwrap()
    {
        if let Ok(code) = process_response(
            &filenode.expect("Problem getting direntry!").path(),
        ) {
            let mut outfile = std::fs::OpenOptions::new()
                .append(true)
                .open(output_path())
                .unwrap();
            outfile.write_all(code.to_string().as_bytes()).unwrap();
            assert!(std::process::Command::new("rustfmt")
                .arg(output_path())
                .output()
                .unwrap()
                .status
                .success());
        } else {
            todo!("Holy moly something is messed up!");
        }
    }
}

fn process_response(
    file: &std::path::Path,
) -> TypegenResult<proc_macro2::TokenStream> {
    let acc = proc_macro2::TokenStream::new();
    let (name, file_body) = get_data(file);
    match file_body {
        serde_json::Value::Object(obj) => Ok(typegen(obj, &name, acc)
            .expect(&format!(
                "file_body of {} struct failed to match",
                file.to_str().unwrap()
            ))
            .1),
        val => Ok(alias(val, &name, acc).expect(&format!(
            "file_body of {} alias failed to match",
            file.to_str().unwrap()
        ))),
    }
}

fn get_data(file: &std::path::Path) -> (String, serde_json::Value) {
    let file_body =
        from_file_deserialize(&file).expect("Couldn't unpack file!");
    let mut name = capitalize_first_char(
        file.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .strip_suffix(".json")
            .unwrap(),
    );
    name.push_str("Response");
    (name, file_body)
}

/// This function provides input for the OS interface that we access via
/// std::process, and std::fs.
fn output_path() -> std::ffi::OsString {
    std::ffi::OsString::from(
        std::env::args()
            .nth(2)
            .unwrap_or("./../src/client/rpc_response_types.rs".to_string()),
    )
}

/// Handles data access from fs location through deserialization
fn from_file_deserialize(
    file_path: &std::path::Path,
) -> TypegenResult<serde_json::Value> {
    let from_io_to_fs = error::FSError::from_io_error(file_path);
    let mut file = std::fs::File::open(file_path).map_err(&from_io_to_fs)?;
    let mut file_body = String::new();
    use std::io::Read as _;
    file.read_to_string(&mut file_body)
        .map_err(&from_io_to_fs)?;
    let file_body_json =
        serde_json::de::from_str(&file_body).map_err(|err| {
            error::JsonError::from_serde_json_error(err, file_body)
        })?;
    Ok(file_body_json)
}

/// Simple wrapper that always generates Idents with "call_site" spans.
fn callsite_ident(name: &str) -> proc_macro2::Ident {
    proc_macro2::Ident::new(name, proc_macro2::Span::call_site())
}

fn typegen(
    inner_node: serde_json::Map<String, serde_json::Value>,
    name: &str,
    mut acc: proc_macro2::TokenStream,
) -> TypegenResult<(Option<special_cases::Case>, proc_macro2::TokenStream)> {
    let mut code = Vec::new();
    let mut standalone = None;
    // The default collection behind a serde_json_map is a BTreeMap
    // and being the predicate of "in" causes into_iter to be called.
    // See: https://docs.serde.rs/src/serde_json/map.rs.html#3
    for (mut field_name, val) in inner_node {
        dbg!(&field_name);
        //special case handling
        if &field_name == "xxxx" {
            acc = tokenize_value(name, val, acc)?.1; //We ignore the first field
            return Ok((Some(special_cases::Case::FourXs), acc));
        }

        if special_cases::RESERVED_KEYWORDS.contains(&field_name.as_str()) {
            todo!("Field name with reserved keyword: {}", field_name);
        }

        if field_name.starts_with("alsoStandalone<") {
            field_name = field_name
                .trim_end_matches(">")
                .trim_start_matches("alsoStandalone<")
                .to_string();
            standalone = Some(None);
        };

        let (mut val, temp_acc) =
            tokenize_value(&capitalize_first_char(&field_name), val, acc)?;
        acc = temp_acc;

        if let Some(None) = standalone {
            standalone = Some(Some(val.clone()));
        }

        if field_name.starts_with("Option<") {
            field_name = field_name
                .trim_end_matches(">")
                .trim_start_matches("Option<")
                .to_string();
            use std::str::FromStr as _;
            val =
                proc_macro2::TokenStream::from_str(&format!("Option<{}>", val))
                    .unwrap();
        }

        //println!("Got field: {}, {}", field_name, val);
        let key = callsite_ident(&field_name);
        let added_code = quote::quote!(pub #key: #val,);
        code.push(added_code);
    }

    let ident = callsite_ident(name);
    let body = if let Some(Some(variant)) = standalone {
        quote::quote!(
            pub enum #ident {
                Regular(#variant),
                Verbose {
                    #(#code)*
                },
            }
        )
    } else {
        quote::quote!(
            pub struct #ident {
                #(#code)*
            }
        )
    };

    acc.extend(quote::quote!(
        #[derive(Debug, serde::Deserialize, serde::Serialize)]
        #body
    ));
    Ok((None, acc))
}

fn alias(
    data: serde_json::Value,
    name: &str,
    acc: proc_macro2::TokenStream,
) -> TypegenResult<proc_macro2::TokenStream> {
    let ident = callsite_ident(&name);
    let (type_body, mut acc) =
        tokenize_value(&capitalize_first_char(name), data, acc)?;
    let aliased = quote::quote!(
        pub type #ident = #type_body;
    );
    acc.extend(aliased);
    Ok(acc)
}

fn tokenize_value(
    name: &str,
    val: serde_json::Value,
    acc: proc_macro2::TokenStream,
) -> TypegenResult<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
    match val {
        serde_json::Value::String(kind) => {
            tokenize_terminal(name, kind.as_str(), acc)
        }
        serde_json::Value::Array(vec) => tokenize_array(name, vec, acc),
        serde_json::Value::Object(obj) => tokenize_object(name, obj, acc),
        otherwise => Err(error::AnnotationError {
            kind: error::InvalidAnnotationKind::from(otherwise),
            location: name.to_string(),
        })?,
    }
}

fn tokenize_terminal(
    name: &str,
    val: &str,
    acc: proc_macro2::TokenStream,
) -> TypegenResult<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
    Ok((
        match val {
            "Decimal" => quote::quote!(rust_decimal::Decimal),
            "bool" => quote::quote!(bool),
            "String" => quote::quote!(String),
            otherwise => Err(error::AnnotationError {
                kind: error::InvalidAnnotationKind::from(
                    serde_json::Value::String(otherwise.to_string()),
                ),
                location: name.to_string(),
            })?,
        },
        acc,
    ))
}

fn tokenize_array(
    name: &str,
    mut array_of: Vec<serde_json::Value>,
    acc: proc_macro2::TokenStream,
) -> TypegenResult<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
    let (val, acc) = tokenize_value(
        name,
        array_of.pop().ok_or(error::AnnotationError {
            kind: error::InvalidAnnotationKind::EmptyArray,
            location: name.to_string(),
        })?,
        acc,
    )?;
    Ok((quote::quote!(Vec<#val>), acc))
}

fn tokenize_object(
    name: &str,
    val: serde_json::Map<String, serde_json::Value>,
    acc: proc_macro2::TokenStream,
) -> TypegenResult<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
    let ident = callsite_ident(name);
    let (special_case, acc) = typegen(val, name, acc)?;
    if let Some(special_case) = special_case {
        match special_case {
            special_cases::Case::FourXs => Ok((
                quote::quote!(std::collections::HashMap<String, #ident>),
                acc,
            )),
        }
    } else {
        Ok((quote::quote!(#ident), acc))
    }
}

fn capitalize_first_char(input: &str) -> String {
    let mut ret = input.to_string();
    let ch = ret.remove(0);
    ret.insert(0, ch.to_ascii_uppercase());
    ret
}

#[cfg(test)]
mod unit {
    mod atomic {
        use crate::*;
        #[test]
        fn tokenize_value_string() {
            let quoted_string = tokenize_value(
                "some_field",
                serde_json::json!("String"),
                proc_macro2::TokenStream::new(),
            );
            assert_eq!(
                quote::quote!(String).to_string(),
                quoted_string.unwrap().0.to_string(),
            );
        }
        #[test]
        fn tokenize_value_number() {
            let quoted_number = tokenize_value(
                "some_field",
                serde_json::json!("Decimal"),
                proc_macro2::TokenStream::new(),
            );
            assert_eq!(
                quote::quote!(rust_decimal::Decimal).to_string(),
                quoted_number.unwrap().0.to_string(),
            );
        }
        #[test]
        fn tokenize_value_bool() {
            let quoted_bool = tokenize_value(
                "some_field",
                serde_json::json!("bool"),
                proc_macro2::TokenStream::new(),
            );
            assert_eq!(
                quote::quote!(bool).to_string(),
                quoted_bool.unwrap().0.to_string(),
            );
        }
    }
    mod intermediate {
        use crate::*;
        #[test]
        fn process_response_getinfo() {
            let getinfo_path = std::path::Path::new(
                "./test_data/quizface_output/getinfo.json",
            );
            let output = process_response(getinfo_path);
            assert_eq!(
                output.unwrap().to_string(),
                test_consts::GETINFO_RESPONSE
            );
        }
        #[test]
        fn tokenize_object_simple_unnested() {
            let quoted_object = tokenize_value(
                "somefield",
                serde_json::json!(
                    {
                        "inner_a": "String",
                        "inner_b": "bool",
                        "inner_c": "Decimal",
                    }
                ),
                proc_macro2::TokenStream::new(),
            )
            .unwrap();
            assert_eq!(
                quote::quote!(somefield).to_string(),
                quoted_object.0.to_string(),
            );
            assert_eq!(
                quoted_object.1.to_string(),
                test_consts::SIMPLE_UNNESTED_RESPONSE,
            );
        }
    }
}

#[cfg(test)]
mod test_consts {
    pub(super) const GETINFO_RESPONSE: &str = "# [derive (Debug , serde :: \
    Deserialize , serde :: Serialize)] pub struct GetinfoResponse { pub proxy \
    : Option < String > , pub \
    balance : rust_decimal :: Decimal , pub blocks : rust_decimal :: Decimal \
    , pub connections : rust_decimal :: Decimal , pub difficulty : rust_decimal \
    :: Decimal , pub errors : String , pub keypoololdest : rust_decimal :: \
    Decimal , pub keypoolsize : rust_decimal :: Decimal , pub paytxfee : \
    rust_decimal :: Decimal , pub protocolversion : rust_decimal :: Decimal , \
    pub relayfee : rust_decimal :: Decimal , \
    pub testnet : bool , pub timeoffset : rust_decimal :: Decimal , pub \
    unlocked_until : rust_decimal :: Decimal , pub version : rust_decimal :: \
    Decimal , pub walletversion : rust_decimal :: Decimal , }";
    pub(super) const SIMPLE_UNNESTED_RESPONSE: &str = "# [derive (Debug , \
    serde :: Deserialize , serde :: Serialize)] pub struct somefield { pub \
    inner_a : String , pub inner_b : bool , pub inner_c : rust_decimal :: \
    Decimal , }";
}
