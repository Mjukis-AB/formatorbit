# Checksums Plugin for Formatorbit
#
# This plugin adds checksum/hash computations for byte data.
# Bundled with forb as a sample - rename to enable.
#
# Enable with: forb --plugins toggle checksums

__forb_plugin__ = {
    "name": "Checksums",
    "version": "1.0.0",
    "author": "Formatorbit",
    "description": "CRC32, MD5, SHA-1, SHA-256, SHA-512, BLAKE2b checksums for byte data"
}

import forb
import hashlib
import zlib


@forb.trait(id="crc32", name="CRC32", value_types=["bytes"])
def compute_crc32(value):
    """
    Compute CRC32 checksum.

    Example:
        forb "hello"
        -> crc32: 3610a686
    """
    if not isinstance(value, bytes):
        return None
    # Skip very large data (>10MB)
    if len(value) > 10_000_000:
        return None
    crc = zlib.crc32(value) & 0xffffffff
    return f"crc32: {crc:08x}"


@forb.trait(id="md5", name="MD5", value_types=["bytes"])
def compute_md5(value):
    """
    Compute MD5 hash.

    Example:
        forb "hello"
        -> md5: 5d41402abc4b2a76b9719d911017c592
    """
    if not isinstance(value, bytes):
        return None
    if len(value) > 10_000_000:
        return None
    return f"md5: {hashlib.md5(value).hexdigest()}"


@forb.trait(id="sha1", name="SHA-1", value_types=["bytes"])
def compute_sha1(value):
    """
    Compute SHA-1 hash.

    Example:
        forb "hello"
        -> sha1: aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d
    """
    if not isinstance(value, bytes):
        return None
    if len(value) > 10_000_000:
        return None
    return f"sha1: {hashlib.sha1(value).hexdigest()}"


@forb.trait(id="sha256", name="SHA-256", value_types=["bytes"])
def compute_sha256(value):
    """
    Compute SHA-256 hash.

    Example:
        forb "hello"
        -> sha256: 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
    """
    if not isinstance(value, bytes):
        return None
    # Skip large data (>1MB) - SHA-256 is slower
    if len(value) > 1_000_000:
        return None
    return f"sha256: {hashlib.sha256(value).hexdigest()}"


@forb.trait(id="sha512", name="SHA-512", value_types=["bytes"])
def compute_sha512(value):
    """
    Compute SHA-512 hash.

    Example:
        forb "hello"
        -> sha512: 9b71d224bd62f3785d96d46ad3ea3d73319bfbc2890caadae2dff72519673ca7...
    """
    if not isinstance(value, bytes):
        return None
    if len(value) > 1_000_000:
        return None
    return f"sha512: {hashlib.sha512(value).hexdigest()}"


@forb.trait(id="blake2b-256", name="BLAKE2b-256", value_types=["bytes"])
def compute_blake2b(value):
    """
    Compute BLAKE2b-256 hash.

    Example:
        forb "hello"
        -> blake2b-256: 324dcf027dd4a30a932c441f365a25e86b173defa4b8e58948253471b81b72cf
    """
    if not isinstance(value, bytes):
        return None
    if len(value) > 1_000_000:
        return None
    return f"blake2b-256: {hashlib.blake2b(value, digest_size=32).hexdigest()}"
