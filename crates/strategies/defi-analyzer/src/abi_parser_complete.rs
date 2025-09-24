//! Complete ABI Parser for Symbolic Execution
//! 
//! This module provides complete ABI parsing functionality for symbolic execution,
//! maintaining full compatibility with DeFiAligner's ABI parser implementation.

use alloy_primitives::{Address, U256};
use anyhow::Result;
use tracing::{info, debug, warn, error};
use std::collections::HashMap;
use std::fs;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use z3::{Context, Config, ast::{BV, Bool, Ast}};

use crate::{
    types::AnalysisEvent,
    error::{DeFiResult, DeFiAnalyzerError},
};

/// Complete ABI Parser implementation
pub struct ABIParser {
    /// Z3 context
    ctx: z3::Context,
    /// ABI cache
    abi_cache: HashMap<Address, Vec<ABIElement>>,
    /// Configuration
    config: ABIParserConfig,
}

/// ABI Parser configuration
#[derive(Debug, Clone)]
pub struct ABIParserConfig {
    /// Enable caching
    pub enable_caching: bool,
    /// Cache size limit
    pub cache_size_limit: usize,
    /// Enable function signature generation
    pub enable_signature_generation: bool,
}

impl Default for ABIParserConfig {
    fn default() -> Self {
        Self {
            enable_caching: true,
            cache_size_limit: 1000,
            enable_signature_generation: true,
        }
    }
}

/// ABI Element structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ABIElement {
    /// Constant flag
    pub constant: Option<bool>,
    /// Payable flag
    pub payable: Option<bool>,
    /// State mutability
    pub state_mutability: Option<String>,
    /// Type
    pub r#type: String,
    /// Name
    pub name: Option<String>,
    /// Inputs
    pub inputs: Option<Vec<ABIParameter>>,
    /// Outputs
    pub outputs: Option<Vec<ABIParameter>>,
}

/// ABI Parameter structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ABIParameter {
    /// Name
    pub name: String,
    /// Type
    pub r#type: String,
    /// Indexed flag (for events)
    pub indexed: Option<bool>,
}

impl ABIParser {
    /// Create new ABI parser
    pub fn new(ctx: z3::Context, config: ABIParserConfig) -> Self {
        Self {
            ctx,
            abi_cache: HashMap::new(),
            config,
        }
    }

    /// Read and parse ABI JSON file
    pub fn read_and_parse_abi(&mut self, file_path: &str) -> DeFiResult<Vec<ABIElement>> {
        let content = fs::read_to_string(file_path)
            .map_err(|e| DeFiAnalyzerError::ConfigurationError {
                reason: format!("Failed to read ABI file: {}", e),
            })?;

        let abi_elements: Vec<ABIElement> = serde_json::from_str(&content)
            .map_err(|e| DeFiAnalyzerError::ConfigurationError {
                reason: format!("Failed to parse ABI JSON: {}", e),
            })?;

        info!("Successfully parsed {} ABI elements from {}", abi_elements.len(), file_path);
        Ok(abi_elements)
    }

    /// Select ABI function by name
    pub fn select_abi_function<'a>(&self, abi_elements: &'a [ABIElement], func_name: &str) -> Option<&'a ABIElement> {
        for element in abi_elements {
            if element.r#type == "function" && element.name.as_ref().map_or(false, |n| n == func_name) {
                return Some(element);
            }
        }
        None
    }

    /// Get ABI elements by type
    pub fn get_abi_elements_by_type<'a>(&self, abi_elements: &'a [ABIElement], element_type: &str) -> Vec<&'a ABIElement> {
        abi_elements.iter()
            .filter(|element| element.r#type == element_type)
            .collect()
    }

    /// Generate Ethereum function hash
    pub fn generate_ethereum_function_hash(&self, abi_element: &ABIElement) -> DeFiResult<String> {
        let empty_vec = Vec::new();
        let inputs = abi_element.inputs.as_ref().unwrap_or(&empty_vec);
        let input_types: Vec<String> = inputs.iter()
            .map(|input| input.r#type.clone())
            .collect();
        
        let signature = format!("{}({})", 
            abi_element.name.as_ref().unwrap_or(&"".to_string()),
            input_types.join(",")
        );

        let mut hasher = Keccak256::new();
        hasher.update(signature.as_bytes());
        let hash_bytes = hasher.finalize();
        let function_hash = format!("0x{}", hex::encode(&hash_bytes[..4]));

        debug!("Generated function hash for {}: {}", signature, function_hash);
        Ok(function_hash)
    }

    /// Convert ABI type to Z3 value
    pub fn abi_type_to_z3_value(&self, abi_type: &str, name: &str) -> DeFiResult<z3::ast::BV> {
        let ctx = &self.ctx;
        
        match abi_type {
            t if t.starts_with("uint") || t.starts_with("int") => {
                Ok(BV::new_const(ctx, name, 256))
            },
            "address" => {
                Ok(BV::new_const(ctx, name, 256))
            },
            "bool" => {
                Ok(BV::new_const(ctx, name, 256))
            },
            t if t.starts_with("bytes") && t != "bytes" => {
                Ok(BV::new_const(ctx, name, 256))
            },
            "string" => {
                Ok(BV::new_const(ctx, name, 256))
            },
            _ => {
                // Handle complex types like arrays, structs, etc.
                warn!("Unsupported ABI type: {}, using default array location", abi_type);
                let array_location = BV::from_u64(ctx, 0x80, 256);
                let zero_data = BV::from_u64(ctx, 0, 256);
                Ok(array_location.concat(&zero_data))
            }
        }
    }

    /// Generate symbolic input based on ABI
    pub fn generate_symbolic_input<'a>(&self, abi_element: &ABIElement, 
                                  specified_values: Option<HashMap<String, z3::ast::BV<'a>>>) -> DeFiResult<(z3::ast::BV<'a>, HashMap<String, z3::ast::BV<'a>>)> {
        let function_hash_str = self.generate_ethereum_function_hash(abi_element)?;
        let function_hash_decimal = i64::from_str_radix(&function_hash_str[2..], 16)
            .map_err(|e| DeFiAnalyzerError::ConfigurationError {
                reason: format!("Failed to parse function hash: {}", e),
            })?;

        let ctx = &self.ctx;
        let mut symbolic_input_data = BV::from_i64(ctx, function_hash_decimal, 32);

        let mut input_z3_variables = HashMap::new();
        let empty_vec = Vec::new();
        let inputs = abi_element.inputs.as_ref().unwrap_or(&empty_vec);
        
        for input in inputs {
            let value = if let Some(specified_vals) = &specified_values {
                if let Some(specified_val) = specified_vals.get(&input.name) {
                    specified_val.clone()
                } else {
                    self.abi_type_to_z3_value(&input.r#type, &input.name)?
                }
            } else {
                self.abi_type_to_z3_value(&input.r#type, &input.name)?
            };

            let simplified_value = value.simplify();
            input_z3_variables.insert(input.name.clone(), simplified_value.clone());
            symbolic_input_data = symbolic_input_data.concat(&simplified_value);
        }

        let final_input = symbolic_input_data.simplify();
        debug!("Generated symbolic input: {} bytes", final_input.to_string().len());
        
        Ok((final_input, input_z3_variables))
    }

    /// Generate function signature
    pub fn generate_function_signature(&self, abi_element: &ABIElement) -> DeFiResult<String> {
        let empty_vec = Vec::new();
        let inputs = abi_element.inputs.as_ref().unwrap_or(&empty_vec);
        let input_types: Vec<String> = inputs.iter()
            .map(|input| input.r#type.clone())
            .collect();
        
        let signature = format!("{}({})", 
            abi_element.name.as_ref().unwrap_or(&"".to_string()),
            input_types.join(",")
        );
        
        Ok(signature)
    }

    /// Parse ABI from JSON string
    pub fn parse_abi_from_json(&self, json_str: &str) -> DeFiResult<Vec<ABIElement>> {
        let abi_elements: Vec<ABIElement> = serde_json::from_str(json_str)
            .map_err(|e| DeFiAnalyzerError::ConfigurationError {
                reason: format!("Failed to parse ABI JSON: {}", e),
            })?;

        Ok(abi_elements)
    }

    /// Get function by signature
    pub fn get_function_by_signature<'a>(&self, abi_elements: &'a [ABIElement], signature: &str) -> Option<&'a ABIElement> {
        for element in abi_elements {
            if element.r#type == "function" {
                if let Ok(element_signature) = self.generate_function_signature(element) {
                    if element_signature == signature {
                        return Some(element);
                    }
                }
            }
        }
        None
    }

    /// Get event by signature
    pub fn get_event_by_signature<'a>(&self, abi_elements: &'a [ABIElement], signature: &str) -> Option<&'a ABIElement> {
        for element in abi_elements {
            if element.r#type == "event" {
                if let Ok(element_signature) = self.generate_function_signature(element) {
                    if element_signature == signature {
                        return Some(element);
                    }
                }
            }
        }
        None
    }

    /// Validate ABI element
    pub fn validate_abi_element(&self, element: &ABIElement) -> DeFiResult<()> {
        match element.r#type.as_str() {
            "function" => {
                if element.name.is_none() {
                    return Err(DeFiAnalyzerError::ConfigurationError {
                        reason: "Function must have a name".to_string(),
                    });
                }
            },
            "event" => {
                if element.name.is_none() {
                    return Err(DeFiAnalyzerError::ConfigurationError {
                        reason: "Event must have a name".to_string(),
                    });
                }
            },
            "constructor" => {
                // Constructor validation
            },
            "fallback" => {
                // Fallback validation
            },
            "receive" => {
                // Receive validation
            },
            _ => {
                return Err(DeFiAnalyzerError::ConfigurationError {
                    reason: format!("Unknown ABI element type: {}", element.r#type),
                });
            }
        }
        
        Ok(())
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> (usize, usize) {
        (self.abi_cache.len(), self.config.cache_size_limit)
    }

    /// Clear cache
    pub fn clear_cache(&mut self) {
        self.abi_cache.clear();
        info!("ABI cache cleared");
    }

    /// Load ABI from cache or file
    pub fn load_abi(&mut self, address: Address, file_path: &str) -> DeFiResult<Vec<ABIElement>> {
        if self.config.enable_caching {
            if let Some(cached_abi) = self.abi_cache.get(&address) {
                debug!("Loading ABI from cache for address {:?}", address);
                return Ok(cached_abi.clone());
            }
        }

        let abi_elements = self.read_and_parse_abi(file_path)?;
        
        if self.config.enable_caching {
            if self.abi_cache.len() >= self.config.cache_size_limit {
                // Remove oldest entries (simple FIFO)
                let keys_to_remove: Vec<Address> = self.abi_cache.keys().take(self.config.cache_size_limit / 2).cloned().collect();
                for key in keys_to_remove {
                    self.abi_cache.remove(&key);
                }
            }
            
            self.abi_cache.insert(address, abi_elements.clone());
            debug!("Cached ABI for address {:?}", address);
        }

        Ok(abi_elements)
    }
}

/// ABI utility functions
pub struct ABIUtils;

impl ABIUtils {
    /// Check if ABI type is dynamic
    pub fn is_dynamic_type(abi_type: &str) -> bool {
        match abi_type {
            "string" | "bytes" => true,
            t if t.starts_with("bytes") && t != "bytes" => false,
            t if t.contains("[]") => true,
            _ => false,
        }
    }

    /// Get ABI type size in bytes
    pub fn get_type_size(abi_type: &str) -> Option<usize> {
        match abi_type {
            "bool" => Some(1),
            "uint8" | "int8" => Some(1),
            "uint16" | "int16" => Some(2),
            "uint24" | "int24" => Some(3),
            "uint32" | "int32" => Some(4),
            "uint40" | "int40" => Some(5),
            "uint48" | "int48" => Some(6),
            "uint56" | "int56" => Some(7),
            "uint64" | "int64" => Some(8),
            "uint72" | "int72" => Some(9),
            "uint80" | "int80" => Some(10),
            "uint88" | "int88" => Some(11),
            "uint96" | "int96" => Some(12),
            "uint104" | "int104" => Some(13),
            "uint112" | "int112" => Some(14),
            "uint120" | "int120" => Some(15),
            "uint128" | "int128" => Some(16),
            "uint136" | "int136" => Some(17),
            "uint144" | "int144" => Some(18),
            "uint152" | "int152" => Some(19),
            "uint160" | "int160" => Some(20),
            "uint168" | "int168" => Some(21),
            "uint176" | "int176" => Some(22),
            "uint184" | "int184" => Some(23),
            "uint192" | "int192" => Some(24),
            "uint200" | "int200" => Some(25),
            "uint208" | "int208" => Some(26),
            "uint216" | "int216" => Some(27),
            "uint224" | "int224" => Some(28),
            "uint232" | "int232" => Some(29),
            "uint240" | "int240" => Some(30),
            "uint248" | "int248" => Some(31),
            "uint256" | "int256" | "address" => Some(32),
            t if t.starts_with("bytes") && t != "bytes" => {
                let size_str = &t[5..];
                size_str.parse().ok()
            },
            _ => None,
        }
    }

    /// Encode ABI parameters
    pub fn encode_parameters(parameters: &[ABIParameter], values: &[String]) -> DeFiResult<String> {
        // This is a simplified implementation
        // In a real implementation, you would use a proper ABI encoder
        let mut encoded = String::new();
        for (param, value) in parameters.iter().zip(values.iter()) {
            encoded.push_str(&format!("{}:{}", param.name, value));
            if param != parameters.last().unwrap() {
                encoded.push(',');
            }
        }
        Ok(encoded)
    }

    /// Decode ABI parameters
    pub fn decode_parameters(parameters: &[ABIParameter], encoded: &str) -> DeFiResult<Vec<String>> {
        // This is a simplified implementation
        // In a real implementation, you would use a proper ABI decoder
        let values: Vec<String> = encoded.split(',')
            .map(|s| s.split(':').nth(1).unwrap_or("").to_string())
            .collect();
        Ok(values)
    }
}
