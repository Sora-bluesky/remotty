@{
    # Add a version after its implementation is complete.
    # CI permits the same source version before the tag exists.
    # CI requires the remote tag before any later source version is merged.
    RequiredReleases = @(
        "0.2.0"
        "0.2.1"
        "0.2.2"
    )
}
