#!/usr/bin/env python3
"""Stop hook: send Claude back to rewrite a final reply that is not in Japanese.

See "Talk to the owner in Japanese" in the root CLAUDE.md. Kept in the
repository so that cloud sessions, which do not read user settings, run it too.

Code blocks, inline code and URLs are ignored when judging the language, so a
Japanese reply full of commands and paths still passes.
"""
import json
import re
import sys

MIN_LETTERS = 40      # replies with fewer Latin letters than this always pass
MIN_JA_RATIO = 0.15   # share of Japanese characters required among letters

REASON = (
    "直前の返答が日本語になっていません。チャットの返答は日本語で書く決まりです。"
    "同じ内容を日本語で書き直して、返答し直してください。"
)


def main() -> int:
    try:
        data = json.load(sys.stdin)
    except Exception:
        return 0
    if data.get("stop_hook_active"):
        return 0
    text = data.get("last_assistant_message") or ""
    text = re.sub(r"```.*?```", " ", text, flags=re.S)
    text = re.sub(r"`[^`\n]*`", " ", text)
    text = re.sub(r"https?://\S+", " ", text)
    text = re.sub(r"\[[^\]]*\]\([^)]*\)", " ", text)
    latin = len(re.findall(r"[A-Za-z]", text))
    ja = len(re.findall(r"[぀-ヿ㐀-鿿ｦ-ﾟ]", text))
    if latin < MIN_LETTERS:
        return 0
    if ja / (ja + latin) >= MIN_JA_RATIO:
        return 0
    print(json.dumps({"decision": "block", "reason": REASON}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
