import clubscape.account.v1.AccountOuterClass.ClientMessage;
import clubscape.game.v1.Game;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.HexFormat;

/** Synthetic codec/selection/retry checks; no server, shop mutation, or plugin callback runs here. */
public final class ShopIdentityChecks
{
    private static int checks;
    private static final String SESSION = "00000000-0000-4000-8000-000000000001";

    private static void check(boolean condition, String message)
    {
        if (!condition) throw new AssertionError(message);
        checks++;
    }

    private static void reject(Runnable operation)
    {
        try { operation.run(); }
        catch (IllegalArgumentException | IllegalStateException expected) { checks++; return; }
        throw new AssertionError("Invalid new selection accepted");
    }

    private static Game.ShopView view(String item, int index)
    {
        return Game.ShopView.newBuilder().setShop("shop.synthetic.store")
            .addLines(Game.ShopLine.newBuilder().setIndex(index).setItem(item)
                .setStock(1).setBuyPrice(123456).setSellPrice(1)).build();
    }

    private static ClientMessage command(Game.ShopBuy buy)
    {
        return ClientMessage.newBuilder().setProtocolVersion(1).setRequestId(SESSION)
            .setWorldInput(Game.WorldInput.newBuilder().setWorldSessionId(SESSION)
                .setSequence(7).setExpectedCharacterRevision(9).setShopBuy(buy)).build();
    }

    private static byte[] bytes(String prefix, String text, String suffix)
    {
        byte[] before = HexFormat.of().parseHex(prefix), middle = text.getBytes(StandardCharsets.UTF_8);
        byte[] after = HexFormat.of().parseHex(suffix);
        byte[] result = Arrays.copyOf(before, before.length + middle.length + after.length);
        System.arraycopy(middle, 0, result, before.length, middle.length);
        System.arraycopy(after, 0, result, before.length + middle.length, after.length);
        return result;
    }

    private static boolean endsWith(byte[] message, byte[] suffix)
    {
        return message.length >= suffix.length
            && Arrays.equals(Arrays.copyOfRange(message, message.length - suffix.length, message.length), suffix);
    }

    public static void main(String[] args) throws Exception
    {
        byte[][] legacy = {
            bytes("0a14", "shop.synthetic.store", "1801"),
            bytes("0a14", "shop.synthetic.store", "10021832"),
        };
        for (byte[] old : legacy)
        {
            Game.ShopBuy restored = Game.ShopBuy.parseFrom(old);
            check(!restored.hasExpectedItem(), "Historical absence became a present empty identity");
            check(Arrays.equals(restored.toByteArray(), old), "Legacy ShopBuy bytes changed");
            byte[] actionSuffix = new byte[3 + old.length];
            actionSuffix[0] = (byte) 0xca; actionSuffix[1] = 1; actionSuffix[2] = (byte) old.length;
            System.arraycopy(old, 0, actionSuffix, 3, old.length);
            check(endsWith(command(restored).getWorldInput().toByteArray(), actionSuffix), "WorldInput tag25 changed");
            byte[] quote = new byte[2 + old.length];
            quote[0] = 0x1a; quote[1] = (byte) old.length;
            System.arraycopy(old, 0, quote, 2, old.length);
            check(Arrays.equals(Game.QuoteRequest.newBuilder().setShopBuy(restored).build().toByteArray(), quote),
                "QuoteRequest tag3 changed");
            reject(() -> ClubScapeTransport.validateNewShopRequest(command(restored)));
            reject(() -> ClubScapeTransport.validateNewShopRequest(ClientMessage.newBuilder()
                .setPollWorld(Game.PollWorld.newBuilder().setQuote(Game.QuoteRequest.newBuilder().setShopBuy(restored))).build()));
            PendingWorldInput retainedLegacy = new PendingWorldInput();
            retainedLegacy.begin(command(restored));
            check(Arrays.equals(retainedLegacy.original().toByteArray(), command(restored).toByteArray())
                && !retainedLegacy.original().getWorldInput().getShopBuy().hasExpectedItem(),
                "Retry rewrote a historical identity-less intent");
        }
        Game.ShopView displayed = view("item.synthetic.tin", 2);
        ShopSelection selection = ShopSelection.fromDisplayedRow(displayed, 2, 5);
        byte[] expected = bytes("0a14", "shop.synthetic.store", "100218052212");
        byte[] name = "item.synthetic.tin".getBytes(StandardCharsets.UTF_8);
        byte[] selected = Arrays.copyOf(expected, expected.length + name.length);
        System.arraycopy(name, 0, selected, expected.length, name.length);
        check(Arrays.equals(selection.buy().toByteArray(), selected),
            "Expected canonical item is not precisely tag4; no source ID or client price belongs in the request");
        check(selection.quote().getShopBuy().equals(selection.buy()), "Buy and quote selected different identities");
        ClubScapeTransport.validateNewShopRequest(command(selection.buy()));
        ClubScapeTransport.validateNewShopRequest(ClientMessage.newBuilder()
            .setPollWorld(Game.PollWorld.newBuilder().setQuote(selection.quote())).build());
        check(ShopSelection.fromDisplayedRow(view("item.synthetic.pot", 19), 19, 1).buy().getItemIndex() == 19,
            "Sparse displayed indices were mistaken for list offsets");
        reject(() -> ShopSelection.fromDisplayedRow(displayed, 0, 1));
        reject(() -> ShopSelection.fromDisplayedRow(displayed, 2, 0));
        reject(() -> ShopSelection.fromDisplayedRow(displayed.toBuilder().addAllLines(displayed.getLinesList()).build(), 2, 1));
        for (String invalid : new String[]{"", "315", "spawn.synthetic.tin", "item.", "item.invalid id"})
            reject(() -> ShopSelection.fromDisplayedRow(view(invalid, 2), 2, 1));
        Game.ShopBuy presentEmpty = selection.buy().toBuilder().setExpectedItem("").build();
        check(Game.ShopBuy.parseFrom(presentEmpty.toByteArray()).hasExpectedItem(), "Present empty identity lost presence");
        reject(() -> ShopSelection.requireIdentity(presentEmpty));

        byte[] original = command(selection.buy()).toByteArray();
        PendingWorldInput pending = new PendingWorldInput();
        pending.begin(command(selection.buy()));
        Game.ShopView refreshed = view("item.synthetic.pot", 2);
        ShopSelection newChoice = ShopSelection.fromDisplayedRow(refreshed, 2, 5);
        check(selection.buy().getExpectedItem().equals("item.synthetic.tin")
            && newChoice.buy().getExpectedItem().equals("item.synthetic.pot"),
            "Refresh silently replaced the displayed selection");
        reject(() -> pending.begin(command(newChoice.buy())));
        check(Arrays.equals(pending.original().toByteArray(), original), "Uncertain retry changed UUID, sequence, quantity or item");
        check(!pending.rejectedWithoutConsumption(8) && pending.active(), "Advanced sequence incorrectly proved rollback");
        check(Arrays.equals(pending.original().toByteArray(), original), "Reconciliation changed retained bytes");
        check(pending.rejectedWithoutConsumption(7) && !pending.active(), "Validated unconsumed rejection cannot release selection");
        pending.begin(command(newChoice.buy()));
        reject(() -> pending.acknowledged(8));
        pending.acknowledged(7);
        check(!pending.active(), "Matching receipt did not clear retained operation");
        System.out.println("{\"synthetic_shop_identity_checks\":" + checks
            + ",\"live_shop_or_runelite_proof\":false}");
    }
}
