# Default Cryptocurrency Rates Plugin for Formatorbit
#
# This plugin provides live cryptocurrency exchange rates from CoinGecko.
# It's a bundled plugin that ships with forb and is enabled by default.

__forb_plugin__ = {
    "name": "Crypto Rates",
    "version": "1.0.0",
    "author": "Formatorbit",
    "description": "Cryptocurrency exchange rates (BTC, ETH, SOL) from CoinGecko"
}

import forb
import json
from urllib.request import urlopen, Request
from urllib.error import URLError

# Cache rates for 60 seconds to avoid hitting rate limits
_cache = {}
_cache_time = 0
CACHE_DURATION = 60  # seconds

def _fetch_rates():
    """Fetch current rates from CoinGecko API."""
    global _cache, _cache_time
    import time

    now = time.time()
    if _cache and (now - _cache_time) < CACHE_DURATION:
        return _cache

    try:
        url = "https://api.coingecko.com/api/v3/simple/price?ids=bitcoin,ethereum,solana&vs_currencies=usd"
        req = Request(url, headers={"Accept": "application/json"})
        with urlopen(req, timeout=5) as response:
            data = json.loads(response.read().decode())
            _cache = {
                "BTC": data.get("bitcoin", {}).get("usd"),
                "ETH": data.get("ethereum", {}).get("usd"),
                "SOL": data.get("solana", {}).get("usd"),
            }
            _cache_time = now
            return _cache
    except (URLError, json.JSONDecodeError, KeyError):
        return _cache  # Return stale cache on error


@forb.currency(code="BTC", symbol="\u20bf", name="Bitcoin", decimals=8)
def btc_rate():
    """
    Get current Bitcoin to USD exchange rate.

    Example:
        forb "1 BTC"      # Shows BTC value in USD, EUR, SEK, etc.
        forb "100 USD"    # Shows USD value in BTC and other currencies
    """
    rates = _fetch_rates()
    rate = rates.get("BTC")
    if rate is None:
        return None
    return (rate, "USD")  # 1 BTC = rate USD


@forb.currency(code="ETH", symbol="\u039e", name="Ethereum", decimals=8)
def eth_rate():
    """
    Get current Ethereum to USD exchange rate.

    Example:
        forb "10 ETH"
    """
    rates = _fetch_rates()
    rate = rates.get("ETH")
    if rate is None:
        return None
    return (rate, "USD")  # 1 ETH = rate USD


@forb.currency(code="SOL", symbol="S", name="Solana", decimals=9)
def sol_rate():
    """
    Get current Solana to USD exchange rate.

    Example:
        forb "100 SOL"
    """
    rates = _fetch_rates()
    rate = rates.get("SOL")
    if rate is None:
        return None
    return (rate, "USD")  # 1 SOL = rate USD
