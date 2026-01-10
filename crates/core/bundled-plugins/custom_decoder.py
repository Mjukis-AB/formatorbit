# Custom Decoder Plugin Example for Formatorbit
#
# This plugin demonstrates how to create a custom decoder.
# Bundled with forb as a sample - rename to enable.

__forb_plugin__ = {
    "name": "Custom Decoder Example",
    "version": "1.0.0",
    "author": "Formatorbit",
    "description": "Example decoder for a custom format prefix"
}

import forb
from forb import CoreValue, Interpretation

@forb.decoder(
    id="example-format",
    name="Example Custom Format",
    aliases=["example", "ex"]
)
def decode_example(input_str):
    """
    Decode strings that start with "EXAMPLE:" prefix.

    Example:
        forb "EXAMPLE:hello world"
        -> Parsed as custom format with content "hello world"
    """
    prefix = "EXAMPLE:"
    if not input_str.startswith(prefix):
        return []

    content = input_str[len(prefix):]

    return [Interpretation(
        value=CoreValue.String(content),
        confidence=0.95,
        description=f"Example format: {content}"
    )]

@forb.decoder(
    id="rot13-encoded",
    name="ROT13 Encoded Text",
    aliases=["rot13"]
)
def decode_rot13(input_str):
    """
    Detect and decode ROT13 encoded text.
    Only triggers if the input looks like it could be ROT13
    (contains only letters, spaces, and common punctuation).
    """
    import string

    # Quick check: must have at least some letters
    letter_count = sum(1 for c in input_str if c.isalpha())
    if letter_count < 3:
        return []

    # Only try if it's mostly letters and common characters
    valid_chars = set(string.ascii_letters + string.digits + " .,!?'-\"")
    if not all(c in valid_chars for c in input_str):
        return []

    # Decode ROT13
    def rot13(s):
        result = []
        for c in s:
            if 'a' <= c <= 'z':
                result.append(chr((ord(c) - ord('a') + 13) % 26 + ord('a')))
            elif 'A' <= c <= 'Z':
                result.append(chr((ord(c) - ord('A') + 13) % 26 + ord('A')))
            else:
                result.append(c)
        return ''.join(result)

    decoded = rot13(input_str)

    # Only return if the decoded text looks different
    if decoded == input_str:
        return []

    return [Interpretation(
        value=CoreValue.String(decoded),
        confidence=0.3,  # Low confidence since any text could be "ROT13"
        description=f"ROT13 decoded: {decoded}"
    )]
