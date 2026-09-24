import QRCode from "qrcode";
import React, { useEffect, useRef } from "react";

type Props = {
  value: string;
  size?: number;
  type: "link" | "text";
} & React.ComponentProps<'canvas'>

export default function QrCode({
  size,
  type,
  value,
  ...restProps
}: Props): React.ReactElement {
  let canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    QRCode.toCanvas(
      canvasRef.current!,
      value,
      { width: size },
      (e: unknown) => {
        if (!e) return;
        console.error("QR error", e);
      },
    );
  }, [value]);

  if (type === "link") {
    return (
      <a href={value}>
        <canvas ref={canvasRef} {...restProps} />
      </a>
    );
  }

  return <canvas ref={canvasRef} {...restProps} />;
}
