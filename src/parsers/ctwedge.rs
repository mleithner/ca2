use pest::Parser;
use pest::iterators::Pairs;
use pest_derive::Parser;
use crate::{RequestedCA, CASpec};

#[derive(Parser)]
#[grammar = "ctwedge.pest"]
struct CTWedgeParser;

pub fn try_parse_ctwedge(contents: &str, strength: u8) -> Option<RequestedCA> {
    let ctwedge_result = CTWedgeParser::parse(Rule::cit_model, &contents);
    if ctwedge_result.is_ok() {
        return Some(parse_ctwedge(ctwedge_result.unwrap(), strength));
    }
    None
}

fn parse_ctwedge(pairs: Pairs<Rule>, strength: u8) -> RequestedCA {
    // Data for RequestedCA
    let mut first_parameter = true;
    let mut current_parameter_values : Vec<String> = Vec::new();
    let mut parameter_names = Vec::new();
    let mut parameter_values : Vec<Vec<String>>= Vec::new();
    let mut parameter_sizes : Vec<u16> = Vec::new();

    // Traverse parse result to extract data for RequestedCA
    for pair in pairs {
        for inner_pair in pair.into_inner() {
            match inner_pair.as_rule() {
                Rule::id => { /* Model name, ignored */ },
                Rule::parameters => {
                    for parameter in inner_pair.into_inner() {
                        for param_kind in parameter.into_inner() {
                            match param_kind.as_rule() {
                                Rule::enumerative => {
                                    for enum_part in param_kind.into_inner() {
                                        match enum_part.as_rule() {
                                            Rule::id => {
                                                if first_parameter {
                                                    first_parameter = false;
                                                } else {
                                                    parameter_sizes.push(current_parameter_values.len() as u16);
                                                    parameter_values.push(current_parameter_values.clone());
                                                    current_parameter_values = Vec::new();
                                                }
                                                parameter_names.push(enum_part.as_str().to_string());
                                            },
                                            Rule::elements => {
                                                for element in enum_part.into_inner() {
                                                    if element.as_rule() == Rule::element {
                                                       current_parameter_values.push(element.as_str().to_string());
                                                    }
                                                }
                                            },
                                            _ => unreachable!("{:?}", enum_part.as_rule())
                                        }
                                    }
                                },
                                Rule::bool => {
                                    for bool_part in param_kind.into_inner() {
                                        match bool_part.as_rule() {
                                            Rule::id => {
                                                if first_parameter {
                                                    first_parameter = false;
                                                } else {
                                                    parameter_sizes.push(current_parameter_values.len() as u16);
                                                    parameter_values.push(current_parameter_values.clone());
                                                    current_parameter_values = Vec::new();
                                                }
                                                parameter_names.push(bool_part.as_str().to_string());
                                            },
                                            Rule::boolean_bareword => {
                                                current_parameter_values.push(String::from("true"));
                                                current_parameter_values.push(String::from("false"));
                                            },
                                            _ => unreachable!("{:?}", bool_part.as_rule())

                                        }
                                    }
                                },
                                Rule::range => {
                                    unimplemented!("The CTWedge range notation is currently unimplemented.");
                                },
                                _ => unreachable!("{:?}", param_kind.as_rule())
                            }
                        }
                    }
                },
                Rule::constraints => {
                    eprintln!("CTWedge Parser Warning: Constraints are unsupported.");
                }
                Rule::EOI => {},
                _ => unreachable!("{:?}", inner_pair.as_rule())
            }
        }
    }
    // Derive the sorted (descending) parameter sizes
    let mut vs = parameter_sizes.clone();
    vs.sort_by(|a, b| b.cmp(a));

    RequestedCA {
        parameter_names,
        parameter_values,
        parameter_sizes,
        ca_spec: CASpec {
            n: 0,
            t: strength,
            vs
        }
    }
}
