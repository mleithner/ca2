use pest::{Parser};
use pest::iterators::Pairs;
use pest_derive::Parser;
use crate::{RequestedCA, CASpec, CA2Version};

#[derive(Parser)]
#[grammar = "acts.pest"]
struct ActsParser;

pub fn try_parse_acts(contents: &str, strength: u8) -> Option<RequestedCA> {
    let acts_result = ActsParser::parse(Rule::file, &contents);
    if acts_result.is_ok() {
        // This is an ACTS file
        return Some(parse_acts(acts_result.unwrap(), strength));
    }
    None
}

fn parse_acts(pairs: Pairs<Rule>, strength: u8) -> RequestedCA {
    // Data for RequestedCA
    let mut first_parameter = true;
    let mut current_parameter_values = Vec::new();
    let mut parameter_names = Vec::new();
    let mut parameter_values = Vec::new();
    let mut parameter_sizes = Vec::new();

    // Traverse parse result to extract data for RequestedCA
    for pair in pairs {
        for inner_pair in pair.into_inner() {
            match inner_pair.as_rule() {
                Rule::section => {
                    for section in inner_pair.into_inner() {
                        match section.as_rule() {
                            Rule::system_section => {},
                            Rule::parameter_section => {
                                for parameter_section in section.into_inner() {
                                    if parameter_section.as_rule() == Rule::parameters {
                                        for parameter in parameter_section.into_inner() {
                                            for parameter_token in parameter.into_inner() {
                                                match parameter_token.as_rule() {
                                                    Rule::parameter_name => {
                                                        if first_parameter {
                                                            first_parameter = false;
                                                        } else {
                                                            parameter_sizes.push(current_parameter_values.len() as u16);
                                                            parameter_values.push(current_parameter_values.clone());
                                                            current_parameter_values = Vec::new();
                                                        }
                                                        parameter_names.push(parameter_token.as_str().to_string());
                                                    },
                                                    Rule::parameter_type => { /* ignored */ },
                                                    Rule::parameter_values => {
                                                        for parameter_value in parameter_token.into_inner() {
                                                            if parameter_value.as_rule() == Rule::value {
                                                                current_parameter_values.push(parameter_value.as_str().to_string());
                                                            }
                                                        }
                                                    },
                                                    _ => unreachable!()
                                                }
                                            }
                                        }
                                    }

                                    // Last parameter
                                    if current_parameter_values.len() > 0 {
                                        parameter_sizes.push(current_parameter_values.len() as u16);
                                        parameter_values.push(current_parameter_values.clone());
                                    }
                                }
                            }
                            Rule::constraint_section => eprintln!("ACTS Parser Warning: Constraints are unsupported."),
                            Rule::test_set_section => eprintln!("ACTS Parser Warning: Predefined test sets are unsupported."),
                            Rule::relation_section => eprintln!("ACTS Parser Warning: Relations/VCAs are unsupported."),
                            _ => unreachable!("ACTS Parser Warning: Unknown section {:?}", section.as_rule())
                        }
                    }
                },
                Rule::EOI => {},
                _ => unreachable!("{:?}", inner_pair.as_rule())
            };
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
            version: CA2Version::default(),
            n: 0,
            t: strength,
            vs
        }
    }
}
