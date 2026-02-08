import React, { Suspense, Fragment } from "react";

const infiniteThenable = { then() {} };

function Suspender({
  freeze,
  children,
}: {
  freeze: boolean;
  children: React.ReactNode;
}) {
  if (freeze) {
    throw infiniteThenable;
  }
  return <Fragment>{children}</Fragment>;
}

interface Props {
  freeze: boolean;
  children: React.ReactNode;
  placeholder?: React.ReactNode;
}

export function Freeze({ freeze, children, placeholder = null }: Props) {
  return (
    <Suspense fallback={placeholder}>
      <Suspender freeze={freeze}>{children}</Suspender>
    </Suspense>
  );
}
