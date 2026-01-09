# Math Extensions Plugin for Formatorbit
#
# This plugin adds mathematical constants and functions to expressions.
# Bundled with forb and enabled by default.

__forb_plugin__ = {
    "name": "Math Extensions",
    "version": "1.0.0",
    "author": "Formatorbit",
    "description": "Mathematical constants (PI, E, PHI, TAU) and functions (factorial, fib, gcd, lcm)"
}

import forb
import math

# Mathematical constants

@forb.expr_var("PI", description="Pi - ratio of circumference to diameter")
def pi():
    return math.pi

@forb.expr_var("E", description="Euler's number - base of natural logarithm")
def euler():
    return math.e

@forb.expr_var("PHI", description="Golden ratio - (1 + sqrt(5)) / 2")
def phi():
    return (1 + math.sqrt(5)) / 2

@forb.expr_var("TAU", description="Tau - 2 * PI (full circle in radians)")
def tau():
    return math.tau

# Mathematical functions

@forb.expr_func("factorial", description="Calculate n!")
def factorial(n):
    return math.factorial(int(n))

@forb.expr_func("fib", description="Fibonacci number at position n")
def fibonacci(n):
    n = int(n)
    if n <= 1:
        return n
    a, b = 0, 1
    for _ in range(n - 1):
        a, b = b, a + b
    return b

@forb.expr_func("gcd", description="Greatest common divisor of two numbers")
def gcd(a, b):
    return math.gcd(int(a), int(b))

@forb.expr_func("lcm", description="Least common multiple of two numbers")
def lcm(a, b):
    a, b = int(a), int(b)
    return abs(a * b) // math.gcd(a, b) if a and b else 0

@forb.expr_func("isPrime", description="Check if n is prime (returns 1 or 0)")
def is_prime(n):
    n = int(n)
    if n < 2:
        return 0
    for i in range(2, int(n**0.5) + 1):
        if n % i == 0:
            return 0
    return 1

@forb.expr_func("sqrt", description="Square root")
def sqrt(n):
    return math.sqrt(float(n))

@forb.expr_func("log", description="Natural logarithm")
def log(n):
    return math.log(float(n))

@forb.expr_func("log10", description="Base-10 logarithm")
def log10(n):
    return math.log10(float(n))

@forb.expr_func("sin", description="Sine (radians)")
def sin(n):
    return math.sin(float(n))

@forb.expr_func("cos", description="Cosine (radians)")
def cos(n):
    return math.cos(float(n))

@forb.expr_func("tan", description="Tangent (radians)")
def tan(n):
    return math.tan(float(n))
