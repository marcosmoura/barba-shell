import { Battery } from './Battery';
import { Clock } from './Clock';
import { Cpu } from './Cpu';

import * as styles from './Status.styles';

export const Status = () => {
  return (
    <div className={styles.status}>
      <Cpu />
      <Battery />
      <Clock />
    </div>
  );
};
