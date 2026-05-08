# Model Governance Policy

## Principle

LARQL experiments must support multiple model families equally. No single model should dominate the codebase or documentation. This ensures research findings are not model-specific and that the system works across different architectures.

## Supported Models

- **BitNet B1.58 Large**: `data/bitnet_b1_58-large/vindex`
- **Gemma 3 4B IT**: `output/gemma3-4b-v2.vindex`

## Configuration

Use environment variables for model paths to ensure flexibility:

- `_PATH`:Primary vindex path (default varies by experiment)
- `BITNET_VINDEX_PATH`: Bitnet-specific path
- `GEMMA_VINDEX_PATH`: Gemma-specific path
- `BITNET_VINDEX_PATH`: Bitnet-specific path
- `GEMMA_VINDEX_PATH`: Gemma-specific path
- `BITNET_MODEL_ID`: HuggingFace model ID for BitNet extraction
- `GEMMA_MODEL_ID`: HuggingFace model ID for Gemma extraction

Example:
```bash
export _PATH=ata/bitnet_b1_58-large/vindex
# or
VINDEX_PATH=output/gemma3-4b-v2.vindex
```

See `.env.example` for a complete template.

## Experiment Guidelines

### 1. Multi-model Support
When creating new experiments:
- Test with at least two different model families
- Document any model-specific requirements or limitations
- Use environment variables for all model paths
- Avoid hardcoding model-specific constants

### 2. Environment Variables
Never hardcode model paths in code. Use:
```python
vindex_path = os.environ.get("LARQL_VINDEX__PATH", "default/path/to/vindex")
```
Or in shell scripts:LARQL__
```bash
./target/release/larql-server ${VINDEX_PATH:-default/path} --port 8080
```

### 3. Documentation
When documenting examples:
- Show both BitNet and Gemma examples
- Document which models are supported
- Link to this policy document

### 4. CI/CD
Configure test matrices to run against both models:
- BitNet tests: verify bitnet-specific features work
- Gemma tests: verify gemma-specific features work
- Cross-model tests: verify features work across both

## Migration Checklist

When adding new experiments or updating existing ones:

- [ ] Update all hardcoded paths to use environment variables
- [ ] Add .env.example entries if new paths are needed
- [ ] Update AGENTS.md if new model-specific rules are needed
- [ ] Update experiment READMEs to show both models
- [ ] Configure CI to test against both models
- [ ] Document any model-specific limitations

## Model-Specific Notes

### BitNet B1.58 Large
- 1.58-bit quantized model
- Smaller memory footprint
- May have different precision characteristics
- Used in chuk-kv-anatomist app testing

### Gemma 3 4B IT
- Full-precision model
- Larger memory footprint
- Standard reference model
- Used in main test suite (test_vindex_bindings.py)

## Enforcement

Agents working on this codebase must:
1. Check AGENTS.md for model representation rules
2. Use environment variables for all model paths
3. Test with multiple models when adding features
4. Document model-specific requirements clearly

Violations should be flagged in code reviews and corrected before merging.
