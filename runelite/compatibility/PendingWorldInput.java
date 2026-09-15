import clubscape.account.v1.AccountOuterClass.ClientMessage;

/** Retains the exact operation bytes; a refreshed row is never used to reconstruct a retry. */
final class PendingWorldInput
{
    private ClientMessage request;

    void begin(ClientMessage next)
    {
        if (request != null)
            throw new IllegalStateException("Resolve the original uncertain operation before choosing another input");
        if (!next.hasWorldInput() || next.getRequestId().isBlank() || next.getWorldInput().getSequence() <= 0)
            throw new IllegalArgumentException("Expected an identified sequenced world input");
        request = next;
    }

    boolean active() { return request != null; }

    ClientMessage original()
    {
        if (request == null) throw new IllegalStateException("No retained operation");
        return request;
    }

    void acknowledged(long sequence)
    {
        if (original().getWorldInput().getSequence() != sequence)
            throw new IllegalArgumentException("Receipt belongs to another operation");
        request = null;
    }

    boolean rejectedWithoutConsumption(long nextSequence)
    {
        if (original().getWorldInput().getSequence() != nextSequence) return false;
        request = null;
        return true;
    }
}
