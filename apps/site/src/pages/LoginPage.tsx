import { useState, type FormEvent } from "react";
import { ApiError } from "../api/client.ts";
import { useAuth } from "../lib/auth.tsx";
import { navigate } from "../lib/router.ts";

type Mode = "login" | "signup";

export function LoginPage() {
  const { login, signup, user } = useAuth();
  const [mode, setMode] = useState<Mode>("login");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  if (user) {
    return (
      <section className="empty">
        <h1>{user.username} 님으로 로그인했어요</h1>
        <p>게임을 하면 포인트가 쌓여요. 이번 주 랭킹을 확인해 보세요.</p>
        <a className="btn btn--primary" href="#/rankings">
          랭킹 보기
        </a>
      </section>
    );
  }

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      await (mode === "login" ? login(username.trim(), password) : signup(username, password));
      navigate("/");
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "요청을 처리하지 못했습니다.");
      setBusy(false);
    }
  };

  return (
    <form className="auth" onSubmit={submit}>
      <div className="segmented auth__tabs" role="tablist">
        {(
          [
            ["login", "로그인"],
            ["signup", "가입하기"],
          ] as const
        ).map(([m, l]) => (
          <button
            key={m}
            type="button"
            role="tab"
            aria-selected={mode === m}
            className={mode === m ? "is-on" : ""}
            onClick={() => {
              setMode(m);
              setError(null);
            }}
          >
            {l}
          </button>
        ))}
      </div>
      <h1>{mode === "login" ? "로그인" : "가입하기"}</h1>
      <p className="muted">
        {mode === "login"
          ? "로그인하면 게임할 때마다 포인트가 쌓이고 랭킹에 올라요."
          : "아이디는 2~20자 한글·영문·숫자·밑줄, 비밀번호는 8자 이상이에요. 이메일은 받지 않아요."}
      </p>
      <label className="field">
        <span className="field__label">아이디</span>
        <input className="input auth__input" value={username} onChange={(e) => setUsername(e.target.value)} autoComplete="username" required />
      </label>
      <label className="field">
        <span className="field__label">비밀번호</span>
        <input
          className="input auth__input"
          type="password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          autoComplete={mode === "login" ? "current-password" : "new-password"}
          required
        />
      </label>
      {error && (
        <p className="alert" role="alert">
          {error}
        </p>
      )}
      <button type="submit" className="btn btn--primary btn--lg auth__submit" disabled={busy || !username || !password}>
        {busy ? "확인하는 중…" : mode === "login" ? "로그인" : "가입하고 시작하기"}
      </button>
      <p className="hint">비밀번호를 잊으면 되찾을 방법이 없어요. 새 아이디로 가입해 주세요.</p>
    </form>
  );
}
