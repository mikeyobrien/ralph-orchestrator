"""Math utility functions.

This module provides basic mathematical operations for use in the Hats
orchestrator test suite and demonstration purposes.
"""


def add_numbers(a: float, b: float) -> float:
    """Add two numbers together.

    This function performs simple addition of two numeric values,
    supporting both integers and floating-point numbers through
    Python's duck typing.

    Args:
        a: The first number to add.
        b: The second number to add.

    Returns:
        The sum of a and b.

    Examples:
        >>> add_numbers(2, 3)
        5
        >>> add_numbers(1.5, 2.5)
        4.0
        >>> add_numbers(-1, 1)
        0
    """
    return a + b
