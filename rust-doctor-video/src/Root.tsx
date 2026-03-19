import "./index.css";
import { Composition } from "remotion";
import { RustDoctorDemo } from "./Composition";

export const RemotionRoot: React.FC = () => {
  return (
    <Composition
      id="RustDoctorDemo"
      component={RustDoctorDemo}
      durationInFrames={360}
      fps={30}
      width={1280}
      height={720}
    />
  );
};
