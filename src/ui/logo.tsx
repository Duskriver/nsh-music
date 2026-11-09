import { Image } from "nsh";
import logo from "../../icon.png";

/** The nsh-music logo, sized in points. */
export function Logo(props: { size?: number }) {
  const size = () => props.size ?? 32;
  return <Image src={logo} accessibilityElementsHidden style={{ width: size(), height: size() }} />;
}
