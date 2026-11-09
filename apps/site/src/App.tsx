import { AccountMenu } from "./components/AccountMenu.tsx";
import { ThemeToggle } from "./components/ThemeToggle.tsx";
import { LoginPage } from "./pages/LoginPage.tsx";
import { RankingsPage } from "./pages/RankingsPage.tsx";
import { useRoute } from "./lib/router.ts";
import { Lobby } from "./pages/Lobby.tsx";
import { LadderRoom } from "./pages/ladder/LadderRoom.tsx";
import { LadderSetup } from "./pages/ladder/LadderSetup.tsx";
import { BingoRoom } from "./pages/bingo/BingoRoom.tsx";
import { BingoSetup } from "./pages/bingo/BingoSetup.tsx";
import { PinballRace } from "./pages/pinball/PinballRace.tsx";
import { PinballSetup } from "./pages/pinball/PinballSetup.tsx";

export function App() {
  const route = useRoute();
  return (
    <div className="shell">
      <header className="topbar">
        <a href="#/" className="brand" aria-label="한판 홈">
          한판
        </a>
        <div className="topbar__right">
          <AccountMenu />
          <ThemeToggle />
        </div>
      </header>
      <main className="page">
        {route.name === "lobby" && <Lobby />}
        {route.name === "ladder-new" && <LadderSetup />}
        {route.name === "ladder-room" && <LadderRoom key={route.id} id={route.id} />}
        {route.name === "bingo-new" && <BingoSetup />}
        {route.name === "bingo-room" && <BingoRoom key={route.id} id={route.id} />}
        {route.name === "pinball" && <PinballSetup />}
        {route.name === "login" && <LoginPage />}
        {route.name === "rankings" && <RankingsPage />}
        {route.name === "pinball-race" && <PinballRace key={route.id} id={route.id} />}
        {route.name === "not-found" && (
          <section className="empty">
            <h1>없는 페이지예요</h1>
            <p>주소를 다시 확인하거나 게임 목록으로 돌아가세요.</p>
            <a className="btn btn--primary" href="#/">
              게임 목록으로
            </a>
          </section>
        )}
      </main>
    </div>
  );
}
