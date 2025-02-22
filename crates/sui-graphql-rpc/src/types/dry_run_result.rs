// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::base64::Base64;
use super::move_type::MoveType;
use super::transaction_block::TransactionBlock;
use super::transaction_block_kind::programmable::TransactionArgument;
use crate::error::Error;
use async_graphql::*;
use sui_json_rpc_types::{DevInspectResults, SuiExecutionResult};
use sui_types::effects::TransactionEffects as NativeTransactionEffects;
use sui_types::transaction::TransactionData as NativeTransactionData;

#[derive(Clone, Debug, SimpleObject)]
pub(crate) struct DryRunResult {
    /// The error that occurred during dry run execution, if any.
    pub error: Option<String>,
    /// The intermediate results for each command of the dry run execution, including
    /// contents of mutated references and return values.
    pub results: Option<Vec<DryRunEffect>>,
    /// The transaction block representing the dry run execution.
    pub transaction: Option<TransactionBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
pub(crate) struct DryRunEffect {
    /// Changes made to arguments that were mutably borrowed by each command in this transaction.
    pub mutated_references: Option<Vec<DryRunMutation>>,

    /// Return results of each command in this transaction.
    pub return_values: Option<Vec<DryRunReturn>>,
}

#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
pub(crate) struct DryRunMutation {
    pub input: TransactionArgument,

    pub type_: MoveType,

    pub bcs: Base64,
}

#[derive(Clone, Debug, PartialEq, Eq, SimpleObject)]
pub(crate) struct DryRunReturn {
    pub type_: MoveType,

    pub bcs: Base64,
}

impl TryFrom<DevInspectResults> for DryRunResult {
    type Error = crate::error::Error;
    fn try_from(results: DevInspectResults) -> Result<Self, Self::Error> {
        let execution_results = results
            .results
            .ok_or_else(|| {
                Error::Internal("No execution results returned from dev inspect".to_string())
            })?
            .into_iter()
            .map(DryRunEffect::try_from)
            .collect::<Result<Vec<_>, Self::Error>>()?;
        let events = results.events.data.into_iter().map(|e| e.into()).collect();
        let effects: NativeTransactionEffects =
            bcs::from_bytes(&results.raw_effects).map_err(|e| {
                Error::Internal(format!("Unable to deserialize transaction effects: {e}"))
            })?;
        let tx_data: NativeTransactionData = bcs::from_bytes(&results.raw_txn_data)
            .map_err(|e| Error::Internal(format!("Unable to deserialize transaction data: {e}")))?;
        let transaction = Some(TransactionBlock::DryRun {
            tx_data,
            effects,
            events,
        });
        Ok(Self {
            error: results.error,
            results: Some(execution_results),
            transaction,
        })
    }
}

impl TryFrom<SuiExecutionResult> for DryRunEffect {
    type Error = crate::error::Error;

    fn try_from(result: SuiExecutionResult) -> Result<Self, Self::Error> {
        let mutated_references = result
            .mutable_reference_outputs
            .iter()
            .map(|(argument, bcs, type_)| {
                Ok(DryRunMutation {
                    input: (*argument).into(),
                    type_: MoveType::new(type_.clone().try_into()?),
                    bcs: bcs.into(),
                })
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()
            .map_err(|e| {
                Error::Internal(format!(
                    "Failed to parse results returned from dev inspect: {:?}",
                    e
                ))
            })?;
        let return_values = result
            .return_values
            .iter()
            .map(|(bcs, type_)| {
                Ok(DryRunReturn {
                    type_: MoveType::new(type_.clone().try_into()?),
                    bcs: bcs.into(),
                })
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()
            .map_err(|e| {
                Error::Internal(format!(
                    "Failed to parse results returned from dev inspect: {:?}",
                    e
                ))
            })?;
        Ok(Self {
            mutated_references: Some(mutated_references),
            return_values: Some(return_values),
        })
    }
}
