import clubscape.game.v1.Game;

/** One immutable selection from a displayed authoritative shop view, shared by buy and quote. */
final class ShopSelection
{
    private final Game.ShopBuy request;

    private ShopSelection(Game.ShopBuy request) { this.request = request; }

    static ShopSelection fromDisplayedRow(Game.ShopView displayed, int rowIndex, int quantity)
    {
        if (displayed.getShop().isBlank() || rowIndex < 0 || quantity <= 0)
            throw new IllegalArgumentException("Select a displayed shop row and a positive quantity");
        Game.ShopLine selected = null;
        for (Game.ShopLine line : displayed.getLinesList())
        {
            if (line.getIndex() != rowIndex) continue;
            if (selected != null) throw new IllegalArgumentException("Ambiguous displayed shop row");
            selected = line;
        }
        if (selected == null) throw new IllegalArgumentException("Shop row is not in the displayed view");
        Game.ShopBuy request = Game.ShopBuy.newBuilder().setShop(displayed.getShop())
            .setItemIndex(selected.getIndex()).setQuantity(quantity)
            .setExpectedItem(selected.getItem()).build();
        requireIdentity(request);
        return new ShopSelection(request);
    }

    static void requireIdentity(Game.ShopBuy request)
    {
        if (!request.hasExpectedItem() || !request.getExpectedItem().startsWith("item.")
            || request.getExpectedItem().length() <= "item.".length()
            || request.getExpectedItem().chars().anyMatch(Character::isWhitespace))
            throw new IllegalArgumentException("New shop requests require the displayed row's canonical item.id");
    }

    Game.ShopBuy buy() { return request; }
    Game.QuoteRequest quote() { return Game.QuoteRequest.newBuilder().setShopBuy(request).build(); }
}
