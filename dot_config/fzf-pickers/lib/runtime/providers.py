def module(kind):
    import developer
    import git_files
    import macos
    import messages
    import network

    providers = {
        name: provider
        for provider in (git_files, macos, messages, network, developer)
        for name in provider.KINDS
    }
    try:
        return providers[kind]
    except KeyError:
        raise RuntimeError(f"Unknown picker: {kind}") from None
