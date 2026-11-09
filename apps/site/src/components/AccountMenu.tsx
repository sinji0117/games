import { useEffect } from "react";
import { useAuth } from "../lib/auth.tsx";

export function AccountMenu() {
  const { user, ready, bonus, dismissBonus, logout } = useAuth();

  useEffect(() => {
    if (!bonus) return;
    const t = window.setTimeout(dismissBonus, 4000);
    return () => window.clearTimeout(t);
  }, [bonus, dismissBonus]);

  return (
    <div className="account">
      <a className="btn btn--ghost btn--sm" href="#/rankings">
        랭킹
      </a>
      {!ready ? null : user ? (
        <>
          <span className="account__who" title={`${user.username} 님의 포인트`}>
            {user.username} <strong>{user.points.toLocaleString("ko-KR")}점</strong>
          </span>
          <button type="button" className="btn btn--ghost btn--sm" onClick={() => void logout()}>
            로그아웃
          </button>
        </>
      ) : (
        <a className="btn btn--ghost btn--sm" href="#/login">
          로그인
        </a>
      )}
      {bonus && (
        <p className="toast" role="status" onClick={dismissBonus}>
          오늘 첫 방문 +{bonus}점
        </p>
      )}
    </div>
  );
}
