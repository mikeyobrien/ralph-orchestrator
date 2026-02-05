# WebSearch Integration Guide

## Overview

Hats now includes full WebSearch support for the Claude adapter, enabling Claude to search the web for current information, research topics, and access data beyond its knowledge cutoff.

## Features

WebSearch allows Claude to:
- Search for current events and recent news
- Research technical documentation and best practices
- Find up-to-date information about libraries and frameworks
- Gather data from multiple sources
- Access real-time information (weather, stock prices, etc.)

## Configuration

### Default Configuration

WebSearch is **enabled by default** when using the Claude adapter. No additional configuration is required.

### Explicit Configuration

You can explicitly control WebSearch in several ways:

#### 1. Via CLI (Automatic)

When using Hats with Claude, WebSearch is automatically enabled:

```bash
hats -a claude  # WebSearch is enabled by default
```

#### 2. Via Adapter Configuration

```python
from src.hats_orchestrator.adapters.claude import ClaudeAdapter

# Create adapter with WebSearch enabled (default)
adapter = ClaudeAdapter()
adapter.configure(enable_web_search=True)  # This is the default

# Or disable WebSearch if needed
adapter.configure(enable_web_search=False)
```

#### 3. Via Orchestrator

```python
from src.hats_orchestrator.orchestrator import HatsOrchestrator

orchestrator = HatsOrchestrator(
    prompt_file="TASK.md",
    primary_tool="claude"
)

# Claude adapter automatically has WebSearch enabled
orchestrator.run()
```

## Usage Examples

### Example 1: Research Current Topics

```python
adapter = ClaudeAdapter()
adapter.configure(enable_all_tools=True)

response = adapter.execute("""
    Search the web for the latest developments in quantum computing
    and create a summary of the most significant breakthroughs in 2024.
""")
```

### Example 2: Technical Documentation Research

```python
response = adapter.execute("""
    Use WebSearch to find the latest best practices for Python async programming.
    Compare different approaches and provide recommendations.
""", enable_web_search=True)
```

### Example 3: Real-time Information

```python
response = adapter.execute("""
    Search for current weather conditions in major tech hubs:
    - San Francisco
    - Seattle  
    - Austin
    - New York
    
    Also find the current stock prices for major tech companies.
""", enable_all_tools=True)
```

### Example 4: Framework Research

```python
response = adapter.execute("""
    Research the latest features in React 19 and Next.js 15.
    Use WebSearch to find migration guides and breaking changes.
    Create a comparison table of new features.
""")
```

## Combining with Other Tools

WebSearch works seamlessly with other Claude tools:

```python
response = adapter.execute("""
    1. Use WebSearch to find the latest Python web framework benchmarks
    2. Create a comparison table in a file called benchmarks.md
    3. Search local codebase for current framework usage
    4. Provide recommendations based on findings
""", enable_all_tools=True)
```

## Testing WebSearch

Run the included test script to verify WebSearch functionality:

```bash
python test_websearch.py
```

This will test:
- Basic WebSearch functionality
- WebSearch with specific tool lists
- Async WebSearch operations

## Best Practices

1. **Be Specific**: Provide clear search queries for better results
2. **Combine Sources**: Use WebSearch with local file analysis for comprehensive research
3. **Verify Information**: Cross-reference important information from multiple searches
4. **Time-Sensitive Data**: Use WebSearch for current events, prices, and recent developments
5. **Documentation**: Search for official documentation and recent updates

## Security Considerations

When WebSearch is enabled, Claude can:
- Access any publicly available web content
- Make HTTP/HTTPS requests to external sites
- Process and analyze web page content

Consider your security requirements when enabling WebSearch in production environments.

## Troubleshooting

### WebSearch Not Working

1. Verify Claude SDK is installed:
   ```bash
   pip install claude-code-sdk
   ```

2. Check if WebSearch is enabled:
   ```python
   adapter = ClaudeAdapter(verbose=True)
   adapter.configure(enable_web_search=True, enable_all_tools=True)
   ```

3. Test with a simple query:
   ```python
   response = adapter.execute("What is the current date?", enable_web_search=True)
   ```

### Rate Limiting

WebSearch may be subject to rate limits. If you encounter issues:
- Add delays between searches
- Batch related queries together
- Use caching when appropriate

## Advanced Configuration

### Custom Tool Lists with WebSearch

```python
# Enable only specific tools including WebSearch
adapter.configure(
    allowed_tools=['WebSearch', 'Read', 'Write', 'Edit'],
    enable_web_search=True
)
```

### Conditional WebSearch

```python
# Enable WebSearch only for specific tasks
if task_requires_research:
    adapter.configure(enable_web_search=True)
else:
    adapter.configure(enable_web_search=False)
```

## Integration with Hats Workflows

WebSearch enhances Hats's capabilities in various workflows:

1. **Documentation Generation**: Research and create up-to-date documentation
2. **Dependency Updates**: Find latest versions and migration guides
3. **Bug Investigation**: Search for known issues and solutions
4. **Best Practices**: Research current industry standards
5. **API Integration**: Find API documentation and examples

## Performance Tips

- Cache search results when appropriate
- Batch related searches together
- Use specific search queries for faster results
- Combine with local tools for comprehensive analysis

## Future Enhancements

Planned improvements for WebSearch integration:
- Search result caching
- Custom search providers
- Advanced filtering options
- Search history tracking
- Offline fallback options