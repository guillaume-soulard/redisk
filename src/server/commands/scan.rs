use std::future::Future;
use std::pin::Pin;
use crate::server::RediskCommandContext;
use crate::server::commands::Command;

pub struct Scan;

impl Command for Scan {
    fn name(&self) -> String {
        String::from("SCAN")
    }

    fn execute<'a>(
        &self,
        context: &'a mut RediskCommandContext,
    ) -> Pin<Box<dyn Future<Output = Vec<u8>> + Send + 'a>> {
        Box::pin(async move {
            if context.args.len() < 2 {
                return context.redisk_protocol.serialize_error("wrong number of arguments for 'scan' command");
            }
            let cursor: usize = match context.args[1].parse() {
                Ok(c) => c,
                Err(_) => return context.redisk_protocol.serialize_error("invalid cursor"),
            };

            let mut pattern = None;
            let mut count = 10;

            let mut i = 2;
            while i < context.args.len() {
                match context.args[i].to_uppercase().as_str() {
                    "MATCH" => {
                        if i + 1 < context.args.len() {
                            pattern = Some(context.args[i + 1].clone());
                            i += 2;
                        } else {
                            return context.redisk_protocol.serialize_error("syntax error");
                        }
                    }
                    "COUNT" => {
                        if i + 1 < context.args.len() {
                            count = match context.args[i + 1].parse() {
                                Ok(c) => c,
                                Err(_) => return context.redisk_protocol.serialize_error("value is not an integer or out of range"),
                            };
                            i += 2;
                        } else {
                            return context.redisk_protocol.serialize_error("syntax error");
                        }
                    }
                    _ => return context.redisk_protocol.serialize_error("syntax error"),
                }
            }

            let pat = pattern.unwrap();
            let mut serialized_keys = Vec::new();
            let mut cnt = 0;
            for key in context.iter().skip(cursor) {
                let key_name = key.clone().0;
                if key_name.contains(&pat) {
                    serialized_keys.push(context.redisk_protocol.serialize_bulk_string(&key_name));
                }
                cnt += 1;
                if cnt == count {
                    break;
                }
            }
            context.redisk_protocol.serialize_array(&vec![
                context.redisk_protocol.serialize_integer((cursor + cnt) as i64),
                context.redisk_protocol.serialize_array(&serialized_keys)
            ])

        })
    }
}
