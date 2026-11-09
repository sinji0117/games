// 의존성 없는 해시 라우터: #/ , #/ladder/new , #/ladder/{id}
import { useEffect, useState } from "react";

export type Route =
  | { name: "lobby" }
  | { name: "ladder-new" }
  | { name: "ladder-room"; id: string }
  | { name: "bingo-new" }
  | { name: "bingo-room"; id: string }
  | { name: "not-found" };

export function parseRoute(hash: string): Route {
  const path = hash.replace(/^#/, "") || "/";
  if (path === "/") return { name: "lobby" };
  if (path === "/ladder/new") return { name: "ladder-new" };
  if (path === "/bingo/new") return { name: "bingo-new" };
  const m = path.match(/^\/(ladder|bingo)\/([0-9a-f-]{36})$/i);
  if (m) return m[1] === "ladder" ? { name: "ladder-room", id: m[2] } : { name: "bingo-room", id: m[2] };
  return { name: "not-found" };
}

export function useRoute(): Route {
  const [route, setRoute] = useState(() => parseRoute(window.location.hash));
  useEffect(() => {
    const on = () => {
      setRoute(parseRoute(window.location.hash));
      window.scrollTo(0, 0);
    };
    window.addEventListener("hashchange", on);
    return () => window.removeEventListener("hashchange", on);
  }, []);
  return route;
}

export const navigate = (path: string) => {
  window.location.hash = path;
};
