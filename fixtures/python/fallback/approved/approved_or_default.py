# policy-approved: REQ-15 explicit locale fallback per i18n spec
lang = payload.get("lang") or "ja-JP"
