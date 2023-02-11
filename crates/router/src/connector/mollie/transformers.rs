use error_stack::{IntoReport, ResultExt};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    core::errors,
    services,
    types::{self, api, storage::enums},
};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Amount {
    pub currency: String,
    pub value: String,
}

//TODO: Fill the struct with respective fields
#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct MolliePaymentsRequest {
    pub amount: Amount,
    pub description: Option<String>,
    #[serde(rename = "redirectUrl")]
    #[serde(default = "default_redirect_url")]
    pub redirect_url: Option<String>,
}

pub fn default_redirect_url() -> Option<String> {
    Some(String::from("https://hyperswitch.io"))
}

impl TryFrom<&types::PaymentsAuthorizeRouterData> for MolliePaymentsRequest {
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(_item: &types::PaymentsAuthorizeRouterData) -> Result<Self, Self::Error> {
        println!("Piyush : Convert Router to MollieRequest");
        let mut req = Self {
            amount: Amount {
                currency: _item.request.currency.to_string(),
                value: format!("{}.00", _item.request.amount.to_string()),
            },
            description: _item.description.as_ref().map(|data| format!("{data}")),
            redirect_url: _item.return_url.as_ref().map(|data| format!("{data}")), // redirect_url: Some(String::from("https://hyperswitch.io"))
        };

        if req.redirect_url == None {
            req.redirect_url = Some(String::from("https://hyperswitch.io"));
        }

        println!("{:?}", req);
        Ok(req)
    }
}

//TODO: Fill the struct with respective fields
// Auth Struct
pub struct MollieAuthType {
    pub(super) api_key: String,
}

impl TryFrom<&types::ConnectorAuthType> for MollieAuthType {
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(_auth_type: &types::ConnectorAuthType) -> Result<Self, Self::Error> {
        if let types::ConnectorAuthType::HeaderKey { api_key } = _auth_type {
            Ok(Self {
                api_key: api_key.to_string(),
            })
        } else {
            Err(errors::ConnectorError::FailedToObtainAuthType.into())
        }
    }
}
// PaymentsResponse
//TODO: Append the remaining status flags
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MolliePaymentStatus {
    Open,
    Canceled,
    #[default]
    Pending,
    Authorized,
    Expired,
    Failed,
    Paid,
}

impl From<MolliePaymentStatus> for enums::AttemptStatus {
    fn from(item: MolliePaymentStatus) -> Self {
        match item {
            MolliePaymentStatus::Open => Self::AuthenticationPending,
            MolliePaymentStatus::Canceled => Self::Voided,
            MolliePaymentStatus::Pending => Self::Pending,
            MolliePaymentStatus::Authorized => Self::Authorized,
            MolliePaymentStatus::Expired => Self::VoidFailed,
            MolliePaymentStatus::Failed => Self::Failure,
            MolliePaymentStatus::Paid => Self::Charged,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct URL {
    pub href: String,
    #[serde(rename = "type")]
    pub _type: String,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct Link {
    #[serde(rename = "self")]
    pub sf: URL,
    #[serde(default = "get_default_url")]
    pub checkout: URL,
    pub dashboard: URL,
    pub documentation: URL,
}

pub fn get_default_url() -> URL {
    URL {
        href: String::from(""),
        _type: String::from(""),
    }
}

//TODO: Fill the struct with respective fields
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MolliePaymentsResponse {
    resource: String,
    id: String,
    mode: String,
    #[serde(rename = "createdAt")]
    created_at: String,
    status: MolliePaymentStatus,
    amount: Amount,
    description: String,
    #[serde(rename = "redirectUrl")]
    redirect_url: String,
    method: String,
    // metadata: String,
    #[serde(rename = "profileId")]
    profile_id: String,
    #[serde(rename = "_links")]
    links: Link,
}

impl<F, T>
    TryFrom<types::ResponseRouterData<F, MolliePaymentsResponse, T, types::PaymentsResponseData>>
    for types::RouterData<F, T, types::PaymentsResponseData>
{
    type Error = error_stack::Report<errors::ConnectorError>;
    fn try_from(
        item: types::ResponseRouterData<F, MolliePaymentsResponse, T, types::PaymentsResponseData>,
    ) -> Result<Self, Self::Error> {
        println!("Piyush : Convert MollieResponse to RouterResponse");

        let to_be_redirected = item.response.links.checkout.href != "";

        if to_be_redirected {
            let redirection_url = match Some(item.response.links.checkout)
                .map(|data| Url::parse(&data.href))
                .transpose()
                .into_report()
                .change_context(errors::ConnectorError::ResponseHandlingFailed)
                .attach_printable("Could not parse the redirection data")
            {
                Ok(it) => it,
                Err(err) => return Err(err),
            };

            let redirection_data = redirection_url.map(|url| services::RedirectForm {
                url: url.to_string(),
                method: services::Method::Get,
                form_fields: std::collections::HashMap::from_iter(
                    url.query_pairs()
                        .map(|(k, v)| (k.to_string(), v.to_string())),
                ),
            });

            // Ok(Self {
            //     status: enums::AttemptStatus::from(item.response.status),
            //     response: Ok(types::PaymentsResponseData::TransactionResponse {
            //         resource_id: types::ResponseId::ConnectorTransactionId(item.response.id),
            //         redirection_data: None,
            //         redirect: false,
            //         mandate_reference: None,
            //         connector_metadata: None,
            //     }),
            //     ..item.data
            // })

            Ok(Self {
                status: enums::AttemptStatus::from(item.response.status),
                response: Ok(types::PaymentsResponseData::TransactionResponse {
                    resource_id: types::ResponseId::ConnectorTransactionId(item.response.id),
                    redirect: redirection_data.is_some(),
                    redirection_data,
                    mandate_reference: None,
                    connector_metadata: None,
                }),
                ..item.data
            })
        } else {
            Ok(Self {
                status: enums::AttemptStatus::from(item.response.status),
                response: Ok(types::PaymentsResponseData::TransactionResponse {
                    resource_id: types::ResponseId::ConnectorTransactionId(item.response.id),
                    redirection_data: None,
                    redirect: false,
                    mandate_reference: None,
                    connector_metadata: None,
                }),
                ..item.data
            })
        }
    }
}

//TODO: Fill the struct with respective fields
// REFUND :
// Type definition for RefundRequest
#[derive(Default, Debug, Serialize)]
pub struct MollieRefundRequest {}

impl<F> TryFrom<&types::RefundsRouterData<F>> for MollieRefundRequest {
    type Error = error_stack::Report<errors::ParsingError>;
    fn try_from(_item: &types::RefundsRouterData<F>) -> Result<Self, Self::Error> {
        todo!()
    }
}

// Type definition for Refund Response

#[allow(dead_code)]
#[derive(Debug, Serialize, Default, Deserialize, Clone)]
pub enum RefundStatus {
    Succeeded,
    Failed,
    #[default]
    Processing,
}

impl From<RefundStatus> for enums::RefundStatus {
    fn from(item: RefundStatus) -> Self {
        match item {
            RefundStatus::Succeeded => Self::Success,
            RefundStatus::Failed => Self::Failure,
            RefundStatus::Processing => Self::Pending,
            //TODO: Review mapping
        }
    }
}

//TODO: Fill the struct with respective fields
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
pub struct RefundResponse {}

impl TryFrom<types::RefundsResponseRouterData<api::Execute, RefundResponse>>
    for types::RefundsRouterData<api::Execute>
{
    type Error = error_stack::Report<errors::ParsingError>;
    fn try_from(
        _item: types::RefundsResponseRouterData<api::Execute, RefundResponse>,
    ) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl TryFrom<types::RefundsResponseRouterData<api::RSync, RefundResponse>>
    for types::RefundsRouterData<api::RSync>
{
    type Error = error_stack::Report<errors::ParsingError>;
    fn try_from(
        _item: types::RefundsResponseRouterData<api::RSync, RefundResponse>,
    ) -> Result<Self, Self::Error> {
        todo!()
    }
}

//TODO: Fill the struct with respective fields
#[derive(Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct MollieErrorResponse {}
