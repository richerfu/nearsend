use std::mem::ManuallyDrop;

use anyhow::{Result, anyhow, bail};
use ohos_asset_binding::{
    AssetAccessibility, AssetAttr, AssetResultCode, AssetReturnType, AssetTag, AssetValue,
    asset_add, asset_query, asset_remove, asset_update,
};

fn bytes(tag: AssetTag, value: impl Into<Vec<u8>>) -> AssetAttr {
    AssetAttr {
        tag,
        value: AssetValue::Blob(ManuallyDrop::new(value.into())),
    }
}

fn alias_attr(alias: &str) -> AssetAttr {
    bytes(AssetTag::AssetTagAlias, alias.as_bytes())
}

fn check(operation: &str, result: AssetResultCode) -> Result<()> {
    match result {
        AssetResultCode::Success => Ok(()),
        error => Err(anyhow!("AssetStore {operation} failed: {error}")),
    }
}

pub(super) fn write(alias: &str, username: &str, password: &[u8]) -> Result<()> {
    let accessibility: u32 = AssetAccessibility::AssetAccessibilityDeviceFirstUnlocked.into();
    let result = asset_add(vec![
        alias_attr(alias),
        bytes(AssetTag::AssetTagSecret, password),
        bytes(AssetTag::AssetTagDataLabelNormal1, username.as_bytes()),
        AssetAttr {
            tag: AssetTag::AssetTagAccessibility,
            value: AssetValue::U32IntT(accessibility),
        },
    ]);

    match result {
        AssetResultCode::Duplicated => check(
            "update",
            asset_update(
                vec![alias_attr(alias)],
                vec![
                    bytes(AssetTag::AssetTagSecret, password),
                    bytes(AssetTag::AssetTagDataLabelNormal1, username.as_bytes()),
                ],
            ),
        ),
        other => check("add", other),
    }
}

pub(super) fn read(alias: &str) -> Result<Option<(String, Vec<u8>)>> {
    let return_type: u32 = AssetReturnType::AssetReturnAll.into();
    let result = asset_query(vec![
        alias_attr(alias),
        AssetAttr {
            tag: AssetTag::AssetTagReturnType,
            value: AssetValue::U32IntT(return_type),
        },
    ]);
    let results = match result {
        Ok(results) => results,
        Err(AssetResultCode::NotFound) => return Ok(None),
        Err(error) => bail!("AssetStore query failed: {error}"),
    };
    let Some(first) = results.result.first() else {
        return Ok(None);
    };

    let value = |tag| {
        first.attrs.iter().find_map(|attr| {
            if std::mem::discriminant(&attr.tag) != std::mem::discriminant(&tag) {
                return None;
            }
            match &attr.value {
                AssetValue::Blob(blob) => Some(blob.as_slice()),
                _ => None,
            }
        })
    };
    let username = value(AssetTag::AssetTagDataLabelNormal1)
        .ok_or_else(|| anyhow!("AssetStore credential has no username"))?;
    let password = value(AssetTag::AssetTagSecret)
        .ok_or_else(|| anyhow!("AssetStore credential has no password"))?;
    Ok(Some((
        std::str::from_utf8(username)?.to_owned(),
        password.to_vec(),
    )))
}

pub(super) fn delete(alias: &str) -> Result<()> {
    match asset_remove(vec![alias_attr(alias)]) {
        AssetResultCode::Success | AssetResultCode::NotFound => Ok(()),
        error => bail!("AssetStore remove failed: {error}"),
    }
}
